use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, PartialAction, StaticGod,
        build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_DATA_MAIN_SECTION,
            MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH,
            ScoredMove,
        },
        god_power,
        harpies::slide_position_with_custom_blockers,
        mortal::mortal_move_gen,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_step_masks,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            is_stop_on_mate,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

/// Atalanta moves once like a mortal, except that once in the game her worker may keep going for as
/// many extra steps as she likes. Each step is an ordinary move - one level up at most, no domes, no
/// occupied squares - so the turn is a walk over the board that ends wherever she stops, followed by
/// the usual build.
///
/// The walk is not stored in the move. Two walks that end on the same square and build in the same
/// place produce bit-identical `BoardState`s, so the number of distinct *results* is one per
/// (worker, final square, build) triple, which the plain mortal layout already addresses. Triton,
/// Artemis and Stymphalians are encoded the same way for the same reason; the note on `build_triton`
/// spells the argument out.
///
/// The one extra bit is `USE_POWER`, and that one is not optional: spending the power writes a flag
/// into `god_data`, so a one-step move and a longer walk that happen to end on the same square are
/// genuinely different positions rather than duplicate encodings of one.
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const USE_POWER_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const USE_POWER_MASK: MoveData = 1 << USE_POWER_OFFSET;
const _LAYOUT_ASSERT: () =
    assert!(((1 as MoveData) << (USE_POWER_OFFSET + 1)) - 1 & !MOVE_DATA_MAIN_SECTION == 0);

/// The from/to/build fields sit where `MortalMove` puts them, which is what lets the generator hand
/// the whole turn over to `mortal_move_gen` once the power has been spent: those moves come back
/// with `USE_POWER` clear and read correctly as Atalanta moves.
const _MORTAL_LAYOUT_ASSERT: () = {
    assert!(MOVE_FROM_POSITION_OFFSET == 0);
    assert!(MOVE_TO_POSITION_OFFSET == POSITION_WIDTH);
    assert!(BUILD_POSITION_OFFSET == 2 * POSITION_WIDTH);
};

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AtalantaMove(pub MoveData);

impl GodMove for AtalantaMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = vec![PartialAction::SelectWorker(self.move_from_position())];

        // The individual steps are not stored, so the UI sees the walk as one jump - the same
        // simplification Artemis, Triton and Stymphalians make.
        if self.is_use_power() {
            res.push(PartialAction::HeroPower(self.move_from_position()));
        }
        res.push(PartialAction::MoveWorker(self.move_to_position().into()));

        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        // A walk may come back to the square it started from, in which case the mask is empty and
        // the worker simply stays put.
        board.worker_xor(player, self.move_mask());

        if self.is_use_power() {
            board.set_god_data(player, 1);
        }

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, board: &BoardState) -> BitBoard {
        let from = self.move_from_position();
        let to = self.move_to_position();
        let endpoints = BitBoard::as_mask(from) | BitBoard::as_mask(to);

        if !self.is_use_power() {
            return endpoints;
        }

        // A walk can pass over any square at all, so to stop one the opponent has to interact with
        // something in the reachable set - there is no smaller structure to appeal to, the way
        // Triton's paths are confined to the perimeter.
        //
        // This is deliberately over-inclusive: it covers every walk, not just the one this win
        // happens to take, so most of these squares block nothing. The consistency checker skips
        // the "did this block actually remove a win" direction for Atalanta for that reason. It is
        // never under-inclusive, which is the direction that would hide a real blocking move.
        //
        // The opposing god is unknown here, so the walk uses the plain neighbor map and ignores
        // Athena's and Hades' movement restrictions, all of which only ever *shrink* the real
        // reachable set. Harpies is covered too: her slides only ever carry a worker along squares
        // at non-increasing heights, and a plain walk can step down that same line one square at a
        // time, so every square a slide touches is in here already.
        let blockers = (board.workers[0] | board.workers[1] | board.at_least_level_4())
            & !BitBoard::as_mask(from);

        endpoints | get_walk_reachable_squares(board, from, blockers)
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for AtalantaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AtalantaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AtalantaMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_power_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        Self(
            Self::new_basic_move(move_from_position, move_to_position, build_position).0
                | USE_POWER_MASK,
        )
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    pub fn new_winning_power_move(move_from_position: Square, move_to_position: Square) -> Self {
        Self(Self::new_winning_move(move_from_position, move_to_position).0 | USE_POWER_MASK)
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn is_use_power(self) -> bool {
        (self.0 & USE_POWER_MASK) != 0
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for AtalantaMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let star = if self.is_use_power() { "*" } else { "" };

        if self.get_is_winning() {
            write!(f, "{}>{}{}#", move_from, star, move_to)
        } else {
            write!(
                f,
                "{}>{}{}^{}",
                move_from,
                star,
                move_to,
                self.build_position()
            )
        }
    }
}

/// Every square a walk starting on `worker_pos` can pass over or land on, using nothing but the
/// board: plain adjacency and "climb at most one". Used by `get_blocker_board`, which has no god to
/// consult.
fn get_walk_reachable_squares(
    board: &BoardState,
    worker_pos: Square,
    blockers: BitBoard,
) -> BitBoard {
    let open = !blockers & BitBoard::MAIN_SECTION_MASK;

    let mut reached = BitBoard::as_mask(worker_pos);
    let mut frontier = reached;

    while frontier.is_not_empty() {
        let mut next = BitBoard::EMPTY;

        for pos in frontier {
            let height = board.get_height(pos);
            let steps = NEIGHBOR_MAP[pos as usize] & open & !board.height_map[3.min(height + 1)];
            next |= steps & !reached;
        }

        reached |= next;
        frontier = next;
    }

    reached
}

pub(super) fn atalanta_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if state.board.god_data[player as usize] != 0 {
        // The power is spent, and what is left is a mortal - including her check detection, which
        // is exact from here on because no future turn of hers can walk either.
        return mortal_move_gen::<F, MUST_CLIMB>(state, player, key_squares);
    }

    let mut result = persephone_check_result!(atalanta_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let all_blockers = prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen;
    let step_masks = get_step_masks(&prelude);
    let all_win_targets = prelude.exactly_level_3 & prelude.win_mask;

    if is_mate_only::<F>() {
        // Every win of hers is a level 2 -> 3 climb off some square the walk can stand on, so with
        // no level 2 square next to a level 3 one there is nothing to walk towards. Worth spending
        // two mask operations on: the walk below runs per worker and mate queries are hot.
        let springboards = apply_mapping_to_mask(all_win_targets, &NEIGHBOR_MAP)
            & prelude.exactly_level_2
            & !prelude.domes_and_frozen;
        if springboards.is_empty() {
            return result;
        }
    }

    // Walking back to the starting square is legal - the square is empty from the first step
    // onwards, and unlike Artemis, whose card bans returning, Atalanta has no such clause. It is a
    // real option: it lets her build without giving up the square she is standing on.
    //
    // It does mean two workers can produce the same position, though: neither moved, and only the
    // build tells the moves apart. Builds already spent on one stand-still move are not offered to
    // the next worker.
    let mut null_move_builds = BitBoard::EMPTY;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let worker_start_mask = worker_start_state.worker_start_mask;

        // The walk's own start square is empty from the first step onwards, so it is walkable.
        let blockers = all_blockers & !worker_start_mask;
        let walkable = !blockers & BitBoard::MAIN_SECTION_MASK;
        let open_by_height = [
            step_masks[0] & walkable,
            step_masks[1] & walkable,
            step_masks[2] & walkable,
            step_masks[3] & walkable,
        ];

        // Keyed off `walkable` rather than the shared blocker mask: a worker standing on a level 3
        // square can step off it, walk to a level 2 square and climb back onto it, which is a
        // genuine win onto its own starting square.
        let win_targets = all_win_targets & walkable;

        // Aphrodite constrains where the turn *ends*, not the squares walked through.
        let final_destination_mask = if (worker_start_mask & prelude.affinity_area).is_not_empty() {
            walkable & prelude.affinity_area
        } else {
            walkable
        };

        // Index 1 holds walk states that have already climbed at least once. Persephone demands a
        // climb somewhere in the turn, not specifically on the last step, so that has to be carried
        // through the walk. Without her the flag is dead weight and everything stays in index 0.
        let mut expandable = [worker_start_mask, BitBoard::EMPTY];
        let mut seen = expandable;
        let mut destinations = [BitBoard::EMPTY; 2];
        // Where the very first step can land. Those are the moves that cost her nothing; every
        // other destination is only reachable by spending the power.
        let mut single_step_destinations = [BitBoard::EMPTY; 2];
        let mut emitted_wins = BitBoard::EMPTY;
        let mut is_first_step = true;

        while (expandable[0] | expandable[1]).is_not_empty() {
            let mut next = [BitBoard::EMPTY; 2];

            for climbed in 0..2 {
                for pos in expandable[climbed] {
                    let height = prelude.board.get_height(pos);
                    let raw_steps =
                        prelude.standard_neighbor_map[pos as usize] & open_by_height[height];

                    // Climbing from level 2 to level 3 wins on the spot, so those squares are never
                    // walked *through*. That holds even when the win itself is unavailable (out of
                    // Aphrodite's area): the step still ends the turn, so the walk cannot use it.
                    let mut steps = raw_steps;
                    if height == 2 {
                        let wins = raw_steps & win_targets;
                        steps &= !wins;

                        let new_wins = wins & final_destination_mask & !emitted_wins;
                        emitted_wins |= new_wins;
                        for win_pos in new_wins {
                            // A win found on the first step is an ordinary mortal climb and leaves
                            // the power in hand. Anything found later took at least two steps.
                            let action = if is_first_step {
                                AtalantaMove::new_winning_move(worker_start_pos, win_pos)
                            } else {
                                AtalantaMove::new_winning_power_move(worker_start_pos, win_pos)
                            };
                            result.push(ScoredMove::new_winning_move(action.into()));
                            if is_stop_on_mate::<F>() {
                                return result;
                            }
                        }
                    }

                    let landings = if prelude.is_against_harpies {
                        let mut landings = BitBoard::EMPTY;
                        for step in steps {
                            landings |= slide_position_with_custom_blockers(
                                prelude.board,
                                pos,
                                step,
                                blockers,
                            )
                            .to_board();
                        }
                        landings
                    } else {
                        steps
                    };

                    // `height_map[height]` is "at least one level higher than here", and the step
                    // mask already caps the rise at one, so this is exactly the climbing landings.
                    let climbing = landings & prelude.board.height_map[height];
                    let mut arrivals = [BitBoard::EMPTY; 2];
                    if MUST_CLIMB && climbed == 0 {
                        arrivals[1] = climbing;
                        arrivals[0] = landings & !climbing;
                    } else {
                        arrivals[climbed] = landings;
                    }

                    for (climbed_after, arrival) in arrivals.into_iter().enumerate() {
                        destinations[climbed_after] |= arrival;
                        if is_first_step {
                            single_step_destinations[climbed_after] |= arrival;
                        }

                        let fresh = arrival & !seen[climbed_after];
                        seen[climbed_after] |= fresh;
                        // Unlike Triton, every landing earns another step.
                        next[climbed_after] |= fresh;
                    }
                }
            }

            expandable = next;
            is_first_step = false;
        }

        if is_mate_only::<F>() {
            continue;
        }

        // A square we could have won on is never worth reaching without winning, and emitting both
        // would give one encoding two different win flags (a winning move stores no build, so it
        // collides with the same move building on A5).
        let reachable = destinations[MUST_CLIMB as usize] & final_destination_mask & !emitted_wins;
        let free_destinations = single_step_destinations[MUST_CLIMB as usize] & reachable;
        // Walking several steps to a square one step could have reached spends the power for a
        // position she could have had for free, so those are dropped rather than generated: same
        // board, strictly fewer options left. Note this is a comparison between walks that are
        // legal *under the same constraints* - under Persephone a one-step move that does not climb
        // is not on offer, and a longer walk that climbs on the way is still generated.
        let power_destinations = reachable & !free_destinations;

        for (worker_end_pos, is_power) in free_destinations
            .into_iter()
            .map(|s| (s, false))
            .chain(power_destinations.into_iter().map(|s| (s, true)))
        {
            let worker_end_mask = BitBoard::as_mask(worker_end_pos);
            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_state.worker_start_height;

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_mask
                | prelude.domes_and_frozen);
            let mut builds =
                NEIGHBOR_MAP[worker_end_pos as usize] & unblocked_squares & prelude.build_mask;

            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (worker_end_mask & prelude.key_squares).is_not_empty();
                builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched as usize];
            }

            if worker_end_pos == worker_start_pos {
                builds &= !null_move_builds;
                null_move_builds |= builds;
            }

            for worker_build_pos in builds {
                let new_action = if is_power {
                    AtalantaMove::new_power_move(worker_start_pos, worker_end_pos, worker_build_pos)
                } else {
                    AtalantaMove::new_basic_move(worker_start_pos, worker_end_pos, worker_build_pos)
                };

                // No check detection: see the comment on `build_atalanta`.
                result.push(build_scored_move::<F, _>(new_action, false, is_improving));
            }
        }
    }

    result
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(0),
        "x" | "X" => Ok(1),
        _ => Err(format!("Must be either empty string or x")),
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        _ => Some(format!("x")),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => Some(format!("Power available")),
        _ => Some(format!("Power used")),
    }
}

/// While the power is in hand, Atalanta emits no check flags at all.
///
/// A check would mean "some worker can walk its way onto a level 2 square that neighbors a level 3
/// square", which depends on the build being considered and so would have to be re-walked for every
/// (destination, build) pair rather than read off a precomputed reach board like every other god.
/// Worse than Triton, whose walk at least has to stop the moment it leaves the perimeter: hers only
/// stops when she chooses to, so the flag would cost a flood fill per candidate move. Triton and
/// Stymphalians, the other unbounded walkers, already skip check detection on the same grounds. An
/// approximate flag is worse than none: the consistency checker compares it against ground truth in
/// both directions.
///
/// The cost is move ordering and the quiescence extension: her checking moves sort with the quiet
/// ones and are not extended in search, so she plays weaker tactically than she should. Once the
/// power is spent the generator hands off to `mortal_move_gen` and the flags come back exact, which
/// is most of the game in practice.
///
/// She borrows Bellerophon's NNUE weights: also a mortal carrying a single once-per-game flag, and
/// `god_data` means the same thing in both (0 available, 1 spent), so the "power spent" input
/// carries over rather than being dropped the way a Mortal proxy would drop it.
pub const fn build_atalanta() -> GodPower {
    god_power(
        GodName::Atalanta,
        build_god_power_movers!(atalanta_move_gen),
        build_god_power_actions::<AtalantaMove>(),
        11458392017730566143,
        4791020163385478327,
    )
    .with_nnue_god_name(GodName::Bellerophon)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}

#[cfg(test)]
mod tests {
    use crate::{
        board::GameStateBuilder,
        consistency_checker::ConsistencyChecker,
        fen::parse_fen,
        move_verifier::MoveVerifier,
        search::{
            SearchContext, WINNING_SCORE_BUFFER, get_win_reached_search_terminator, negamax_search,
        },
        search_terminators::DynamicMaxDepthSearchTerminator,
        square::Square::*,
        transposition_table::TranspositionTable,
    };

    use super::*;

    /// Every destination the given worker is offered, split by whether reaching it spends the
    /// power.
    fn destinations_from(
        state: &FullGameState,
        player: Player,
        from: Square,
    ) -> (BitBoard, BitBoard) {
        let mut free = BitBoard::EMPTY;
        let mut power = BitBoard::EMPTY;

        for scored in GodName::Atalanta.to_power().get_all_moves(state, player) {
            let action: AtalantaMove = scored.action.into();
            if action.move_from_position() != from {
                continue;
            }

            let mask = BitBoard::as_mask(action.move_to_position());
            if action.is_use_power() {
                power |= mask;
            } else {
                free |= mask;
            }
        }

        (free, power)
    }

    #[test]
    fn test_atalanta_move_fits_in_the_main_section() {
        let action = AtalantaMove::new_basic_move(A1, B2, C3);
        assert_eq!(action.move_from_position(), A1);
        assert_eq!(action.move_to_position(), B2);
        assert_eq!(action.build_position(), C3);
        assert!(!action.is_use_power());
        assert_eq!(action.0 & MOVE_DATA_MAIN_SECTION, action.0);

        let power = AtalantaMove::new_power_move(A1, B2, C3);
        assert_eq!(power.move_from_position(), A1);
        assert_eq!(power.move_to_position(), B2);
        assert_eq!(power.build_position(), C3);
        assert!(power.is_use_power());
        assert_eq!(power.0 & MOVE_DATA_MAIN_SECTION, power.0);
    }

    #[test]
    fn test_atalanta_walks_the_whole_board() {
        // Nothing stops a walk on a flat empty board, so every square is a destination - including
        // the far corner and the square she started on.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .build();

        let (free, power) = destinations_from(&state, Player::One, A5);

        assert_eq!(
            free, NEIGHBOR_MAP[A5 as usize],
            "the free moves are exactly the mortal ones"
        );
        assert_eq!(
            free | power,
            BitBoard::MAIN_SECTION_MASK & !BitBoard::as_mask(E1),
            "everything else on the board is reachable by spending the power"
        );
        assert!(
            power.contains_square(A5),
            "walking back to the start square is a legal way to stand still"
        );
    }

    #[test]
    fn test_atalanta_never_pays_for_a_square_one_step_away() {
        // Spending the power to reach a square a single step would have reached buys the same
        // position with fewer options left, so those walks are not generated at all.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(C3)
            .with_p2_worker(E1)
            .build();

        let (free, power) = destinations_from(&state, Player::One, C3);
        assert_eq!(free, NEIGHBOR_MAP[C3 as usize]);
        assert!((free & power).is_empty());
    }

    #[test]
    fn test_atalanta_walking_spends_the_power_and_stepping_does_not() {
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .build();

        let atalanta = GodName::Atalanta.to_power();
        let mortal = GodName::Mortal.to_power();

        for scored in atalanta.get_all_moves(&state, Player::One) {
            let action: AtalantaMove = scored.action.into();
            if action.move_from_position() != A5 {
                continue;
            }

            let next = state.next_state(atalanta, mortal, scored.action);
            let expected = if action.is_use_power() { 1 } else { 0 };
            assert_eq!(
                next.board.god_data[Player::One as usize],
                expected,
                "{:?} should leave god data {}",
                action,
                expected
            );
        }
    }

    #[test]
    fn test_atalanta_power_is_spent_for_good() {
        // Same empty board, but the flag is already set: from here she is a mortal.
        let state = parse_fen("00000 00000 00000 00000 00000/1/atalanta[x]:A5/mortal:E1").unwrap();

        let (free, power) = destinations_from(&state, Player::One, A5);
        assert_eq!(free, NEIGHBOR_MAP[A5 as usize]);
        assert!(power.is_empty(), "the power cannot be spent twice");
    }

    #[test]
    fn test_atalanta_walk_respects_the_climb_limit() {
        // A wall of level 2 squares boxes a ground level worker in, and no number of steps gets
        // her over it.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(B5, 2)
            .with_height(B4, 2)
            .with_height(A4, 2)
            .build();

        let (free, power) = destinations_from(&state, Player::One, A5);
        assert_eq!(free, BitBoard::EMPTY);
        assert_eq!(power, BitBoard::EMPTY);
    }

    #[test]
    fn test_atalanta_wins_by_walking_to_a_springboard() {
        // E1 is a level 3 square on the far side of the board. She walks the empty ground to D1,
        // which is level 2, and climbs off it in the same turn.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(A1)
            .with_height(D1, 2)
            .with_height(E1, 3)
            .with_height(D2, 1)
            .build();

        let next_states = state.get_next_states_interactive();
        MoveVerifier::new()
            .is_winner(Player::One)
            .with_p1_worker_at(E1)
            .any(&next_states);

        let wins = GodName::Atalanta
            .to_power()
            .get_winning_moves(&state, Player::One);
        assert_eq!(wins.len(), 1);
        let win: AtalantaMove = wins[0].action.into();
        assert!(
            win.is_use_power(),
            "the walk to the springboard costs the power"
        );
    }

    #[test]
    fn test_atalanta_direct_win_keeps_the_power() {
        // A plain mortal climb is still a plain mortal climb, and there is no reason to pay for it.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(A5, 2)
            .with_height(B5, 3)
            .build();

        let wins = GodName::Atalanta
            .to_power()
            .get_winning_moves(&state, Player::One);
        assert_eq!(wins.len(), 1);

        let win: AtalantaMove = wins[0].action.into();
        assert_eq!(win.move_to_position(), B5);
        assert!(!win.is_use_power());
    }

    #[test]
    fn test_atalanta_wins_by_walking_back_onto_her_own_square() {
        // She is standing on the level 3 square already, which only happens because Apollo put her
        // there. Stepping down to B5 and climbing straight back onto A5 is an ordinary level 2 -> 3
        // climb that leaves the worker masks untouched.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Apollo)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(A5, 3)
            .with_height(B5, 2)
            .build();

        let wins = GodName::Atalanta
            .to_power()
            .get_winning_moves(&state, Player::One);
        assert_eq!(wins.len(), 1);

        let win: AtalantaMove = wins[0].action.into();
        assert_eq!(win.move_from_position(), A5);
        assert_eq!(win.move_to_position(), A5);
        assert!(win.is_use_power());

        ConsistencyChecker::new(&state)
            .perform_all_validations()
            .expect("consistency check should pass");
    }

    #[test]
    fn test_atalanta_walks_across_level_3_without_winning() {
        // Stepping from one level 3 square to another is not a climb, so it does not win and the
        // walk carries on through it.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Apollo)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(A5, 3)
            .with_height(B5, 3)
            .with_height(C5, 3)
            .build();

        let (_free, power) = destinations_from(&state, Player::One, A5);
        assert!(
            power.contains_square(C5),
            "the far level 3 square is reachable"
        );
        assert!(
            GodName::Atalanta
                .to_power()
                .get_winning_moves(&state, Player::One)
                .is_empty(),
            "walking along level 3 wins nothing"
        );
    }

    #[test]
    fn test_atalanta_vs_persephone_climb_may_come_mid_walk() {
        // A5 -> A4 is flat, A4 -> A3 climbs, A3 -> A2 comes back down to where the turn started.
        // Persephone is satisfied by the climb happening anywhere in the walk, so A2 is legal even
        // though the turn ends no higher than it began.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Persephone)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(A3, 1)
            .build();

        let (free, power) = destinations_from(&state, Player::One, A5);
        assert_eq!(
            free,
            BitBoard::EMPTY,
            "no single step from A5 climbs, so nothing is free"
        );
        assert!(power.contains_square(A3), "the climb itself is legal");
        assert!(
            power.contains_square(A2),
            "stepping back down past the climb is legal"
        );
    }

    #[test]
    fn test_atalanta_vs_persephone_prefers_the_free_climb() {
        // B5 is a climb she can make in one step, so it is offered without spending the power even
        // though a longer walk could also end there.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Persephone)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(B5, 1)
            .build();

        let (free, power) = destinations_from(&state, Player::One, A5);
        assert!(free.contains_square(B5));
        assert!(!power.contains_square(B5));
    }

    #[test]
    fn test_atalanta_vs_persephone_falls_back_when_no_climb_exists() {
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Persephone)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .build();

        let (free, power) = destinations_from(&state, Player::One, A5);
        assert!((free | power).is_not_empty());
    }

    #[test]
    fn test_atalanta_blocker_board_covers_the_walk() {
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(A1)
            .with_height(D1, 2)
            .with_height(E1, 3)
            .with_height(D2, 1)
            .build();

        let atalanta = GodName::Atalanta.to_power();
        let wins = atalanta.get_winning_moves(&state, Player::One);
        assert_eq!(wins.len(), 1);

        let blockers = atalanta.get_blocker_board(&state.board, wins[0].action);
        // D1 is the springboard, and every square of the walk that gets her there is fair game to
        // stand on or dome.
        assert!(blockers.contains_square(D1));
        assert!(blockers.contains_square(A5));
        assert!(blockers.contains_square(E1));
        assert!(blockers.contains_square(C3));
    }

    #[test]
    fn test_atalanta_blocker_board_of_a_free_win_is_just_the_endpoints() {
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(A5, 2)
            .with_height(B5, 3)
            .build();

        let atalanta = GodName::Atalanta.to_power();
        let wins = atalanta.get_winning_moves(&state, Player::One);
        assert_eq!(wins.len(), 1);

        assert_eq!(
            atalanta.get_blocker_board(&state.board, wins[0].action),
            BitBoard::as_mask(A5) | BitBoard::as_mask(B5)
        );
    }

    #[test]
    fn test_atalanta_consistency_on_a_crowded_board() {
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(E1)
            .with_p2_worker(C3)
            .with_p2_worker(B2)
            .with_height(A5, 2)
            .with_height(B5, 3)
            .with_height(C5, 1)
            .with_height(A4, 2)
            .with_height(D1, 2)
            .with_height(E2, 4)
            .with_height(C3, 1)
            .build();

        ConsistencyChecker::new(&state)
            .perform_all_validations()
            .expect("consistency check should pass");
    }

    #[test]
    fn test_atalanta_consistency_once_the_power_is_spent() {
        let state =
            parse_fen("23100 20000 00100 00004 00020/1/atalanta[x]:A5,E1/mortal:C3,B2").unwrap();

        ConsistencyChecker::new(&state)
            .perform_all_validations()
            .expect("consistency check should pass");
    }

    #[test]
    fn test_atalanta_search_finds_the_walking_win() {
        // Exercises the whole search path - move ordering, make/unmake, eval - with a god whose
        // move list contains stand-still moves.
        let state = GameStateBuilder::new(GodName::Atalanta, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(B5)
            .with_p2_worker(A1)
            .with_p2_worker(B1)
            .with_height(D1, 2)
            .with_height(E1, 3)
            .with_height(D2, 1)
            .build();

        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            new_best_move_callback: Box::new(move |_new_best_move| {}),
            terminator: DynamicMaxDepthSearchTerminator::new(2),
        };
        let search_state = negamax_search(
            &mut search_context,
            state,
            get_win_reached_search_terminator(),
        );

        assert!(search_state.best_move.unwrap().score > WINNING_SCORE_BUFFER);
    }
}
