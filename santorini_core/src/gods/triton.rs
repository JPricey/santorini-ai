use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, PERIMETER_SPACES_MASK, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
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
        move_helpers::{
            GeneratorPreludeState, build_scored_move, get_generator_prelude_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            is_stop_on_mate,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

/// Triton's turn is a *chain* of steps - every time he lands on a perimeter space he may step
/// again - but the move is still encoded with the plain mortal from/to/build layout, with no room
/// reserved for the path.
///
/// That is deliberate. The path is not observable in the resulting position: two chains that end on
/// the same square and build in the same place produce bit-identical `BoardState`s, and the
/// consistency checker rejects two encodings that reach one state as duplicate moves. So the
/// interesting question is not "how many chain steps fit in 30 bits" but "how many *distinct
/// results* are there", and the answer is one per (worker, final square, build) triple - which is
/// exactly what the mortal layout addresses. Artemis and Stymphalians, the other multi-step movers,
/// are encoded the same way for the same reason.
///
/// Consequently there is no chain-length cap to justify: generation walks the reachable set to a
/// fixpoint over 25 squares rather than unrolling a bounded number of steps, so a worker that can
/// circle the whole board is represented exactly.
///
/// The one thing the path is genuinely needed for is `get_blocker_board`, and that recomputes a
/// superset of it from the board - see there.
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const _LAYOUT_ASSERT: () = assert!(
    ((1 as MoveData) << (BUILD_POSITION_OFFSET + POSITION_WIDTH)) - 1 & !MOVE_DATA_MAIN_SECTION
        == 0
);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct TritonMove(pub MoveData);

impl GodMove for TritonMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        // The individual steps are not stored, so the UI sees the chain as one jump - the same
        // simplification Artemis and Stymphalians make.
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        // A chain may walk back onto the square it started from, in which case the mask is empty
        // and the worker simply stays put.
        board.worker_xor(player, self.move_mask());

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, board: &BoardState) -> BitBoard {
        let from = self.move_from_position();
        let to = self.move_to_position();

        // To stop the win an opponent has to interact with a square the chain walks over, and every
        // intermediate square of a chain is a perimeter square by definition - a chain that lands
        // anywhere else ends there. So the whole path lives in `from | to | perimeter`, and
        // restricting that to the squares actually reachable from `from` keeps it to the ring
        // segment that matters.
        //
        // This is deliberately over-inclusive: it covers every path, not just the one this win
        // happens to take, so most of these squares block nothing. The consistency checker skips
        // the "did this block actually remove a win" direction for Triton for that reason. It is
        // never under-inclusive, which is the direction that would hide a real blocking move.
        //
        // The opposing god is unknown here, so the walk uses the plain neighbor map and ignores
        // Athena's and Hades' movement restrictions. All of those only ever *shrink* the real
        // reachable set, so the result stays a superset - except against Harpies, whose slides can
        // carry a worker past a non-perimeter square. Harpies covers that from her own side by
        // treating every square as a key square against Triton.
        let blockers = (board.workers[0] | board.workers[1] | board.at_least_level_4())
            & !BitBoard::as_mask(from);

        let reachable = get_chain_reachable_squares(board, from, blockers);

        BitBoard::as_mask(from) | BitBoard::as_mask(to) | (reachable & PERIMETER_SPACES_MASK)
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for TritonMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for TritonMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl TritonMove {
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

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;

        Self(data)
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

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for TritonMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();

        if self.get_is_winning() {
            write!(f, "{}>{}#", move_from, move_to)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, self.build_position())
        }
    }
}

/// Every square a chain starting on `worker_pos` can walk over or land on, using nothing but the
/// board: plain adjacency, "climb at most one", and the perimeter rule that decides whether the
/// chain may continue. Used by `get_blocker_board`, which has no god to consult.
fn get_chain_reachable_squares(
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
        // Only a landing on the perimeter earns another step.
        frontier = next & PERIMETER_SPACES_MASK;
    }

    reached
}

/// Squares a worker may step onto, indexed by the height it is standing on. Folds in Athena's climb
/// ban and Hades' fall ban so the chain walk itself stays simple.
fn get_step_masks(prelude: &GeneratorPreludeState) -> [BitBoard; 4] {
    let height_map = &prelude.board.height_map;

    if prelude.is_down_prevented {
        [
            !height_map[1],
            height_map[0] & !height_map[2],
            height_map[1] & !height_map[3],
            height_map[2] & !height_map[3],
        ]
    } else if prelude.can_climb {
        [
            !height_map[1],
            !height_map[2],
            !height_map[3],
            !height_map[3],
        ]
    } else {
        [
            !height_map[0],
            !height_map[1],
            !height_map[2],
            !height_map[3],
        ]
    }
}

pub(super) fn triton_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(triton_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let all_blockers = prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen;
    let step_masks = get_step_masks(&prelude);
    let all_win_targets = prelude.exactly_level_3 & prelude.win_mask;

    if is_mate_only::<F>() {
        // Every Triton win is a level 2 -> 3 climb off some square the chain can stand on, so with
        // no level 2 square next to a level 3 one there is nothing to walk towards. Worth spending
        // two mask operations on: the walk below runs per worker and mate queries are hot.
        let springboards = apply_mapping_to_mask(all_win_targets, &NEIGHBOR_MAP)
            & prelude.exactly_level_2
            & !prelude.domes_and_frozen;
        if springboards.is_empty() {
            return result;
        }
    }

    // Ending back on the starting square is legal and generated. Nothing in the power forbids it -
    // the square is empty from the first step onwards - and unlike Artemis, whose card explicitly
    // bans returning, Triton has no such clause. It is a real option too: it lets him build without
    // giving up the square he is standing on.
    //
    // It does mean two workers can produce the same position, though: neither moved, and only the
    // build tells the moves apart. Builds already spent on one stand-still move are not offered to
    // the next worker.
    let mut null_move_builds = BitBoard::EMPTY;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let worker_start_mask = worker_start_state.worker_start_mask;

        // The chain's own start square is empty from the first step onwards, so it is walkable.
        let blockers = all_blockers & !worker_start_mask;
        let walkable = !blockers & BitBoard::MAIN_SECTION_MASK;
        let open_by_height = [
            step_masks[0] & walkable,
            step_masks[1] & walkable,
            step_masks[2] & walkable,
            step_masks[3] & walkable,
        ];

        // Keyed off `walkable` rather than the shared blocker mask: a worker standing on a level 3
        // square can step off it, walk to a level 2 perimeter square and climb back onto it, which
        // is a genuine win onto its own starting square.
        let win_targets = all_win_targets & walkable;

        // Aphrodite constrains where the turn *ends*, not the squares walked through.
        let final_destination_mask = if (worker_start_mask & prelude.affinity_area).is_not_empty() {
            walkable & prelude.affinity_area
        } else {
            walkable
        };

        // Index 1 holds chain states that have already climbed at least once. Persephone demands a
        // climb somewhere in the turn, not specifically on the last step, so that has to be carried
        // through the walk. Without her the flag is dead weight and everything stays in index 0.
        let mut expandable = [worker_start_mask, BitBoard::EMPTY];
        let mut seen = expandable;
        let mut destinations = [BitBoard::EMPTY; 2];
        let mut emitted_wins = BitBoard::EMPTY;

        while (expandable[0] | expandable[1]).is_not_empty() {
            let mut next = [BitBoard::EMPTY; 2];

            for climbed in 0..2 {
                for pos in expandable[climbed] {
                    let height = prelude.board.get_height(pos);
                    let raw_steps =
                        prelude.standard_neighbor_map[pos as usize] & open_by_height[height];

                    // Climbing from level 2 to level 3 wins on the spot, so those squares are never
                    // walked *through*. That holds even when the win itself is unavailable (out of
                    // Aphrodite's area): the step still ends the turn, so the chain cannot use it.
                    let mut steps = raw_steps;
                    if height == 2 {
                        let wins = raw_steps & win_targets;
                        steps &= !wins;

                        let new_wins = wins & final_destination_mask & !emitted_wins;
                        emitted_wins |= new_wins;
                        for win_pos in new_wins {
                            result.push(ScoredMove::new_winning_move(
                                TritonMove::new_winning_move(worker_start_pos, win_pos).into(),
                            ));
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

                        let fresh = arrival & !seen[climbed_after];
                        seen[climbed_after] |= fresh;
                        next[climbed_after] |= fresh & PERIMETER_SPACES_MASK;
                    }
                }
            }

            expandable = next;
        }

        if is_mate_only::<F>() {
            continue;
        }

        // A square we could have won on is never worth reaching without winning, and emitting both
        // would give one 15 bit encoding two different win flags (a winning move stores no build,
        // so it collides with the same move building on A5).
        let final_destinations =
            destinations[MUST_CLIMB as usize] & final_destination_mask & !emitted_wins;

        for worker_end_pos in final_destinations {
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
                let new_action =
                    TritonMove::new_basic_move(worker_start_pos, worker_end_pos, worker_build_pos);

                // No check detection: see the comment on `build_triton`.
                result.push(build_scored_move::<F, _>(new_action, false, is_improving));
            }
        }
    }

    result
}

/// Triton emits no check flags at all.
///
/// A check would mean "some worker can chain its way onto a level 2 square that neighbors a level 3
/// square", which depends on the build being considered and so would have to be re-walked for every
/// (destination, build) pair rather than read off a precomputed reach board like every other god.
/// The project's own notes call this out as the hard part of Triton and sanction skipping it, and
/// Stymphalians - the other unbounded walker - already skips it on the same grounds. An approximate
/// flag is worse than none: the consistency checker compares it against ground truth in both
/// directions.
///
/// The cost is move ordering and the quiescence extension: Triton's checking moves sort with the
/// quiet ones and are not extended in search, so he plays weaker tactically than he should.
pub const fn build_triton() -> GodPower {
    god_power(
        GodName::Triton,
        build_god_power_movers!(triton_move_gen),
        build_god_power_actions::<TritonMove>(),
        6157020147743148521,
        15188297704320771830,
    )
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::{
        board::GameStateBuilder,
        consistency_checker::ConsistencyChecker,
        move_verifier::MoveVerifier,
        search::{
            SearchContext, WINNING_SCORE_BUFFER, get_win_reached_search_terminator, negamax_search,
        },
        search_terminators::DynamicMaxDepthSearchTerminator,
        square::Square::*,
        transposition_table::TranspositionTable,
    };

    use super::*;

    fn destinations_from(state: &FullGameState, player: Player, from: Square) -> BitBoard {
        let mut res = BitBoard::EMPTY;
        for scored in GodName::Triton.to_power().get_all_moves(state, player) {
            let action: TritonMove = scored.action.into();
            if action.move_from_position() == from {
                res |= BitBoard::as_mask(action.move_to_position());
            }
        }
        res
    }

    #[test]
    fn test_triton_move_fits_in_the_main_section() {
        let action = TritonMove::new_basic_move(A1, B2, C3);
        assert_eq!(action.move_from_position(), A1);
        assert_eq!(action.move_to_position(), B2);
        assert_eq!(action.build_position(), C3);
        assert_eq!(action.0 & MOVE_DATA_MAIN_SECTION, action.0);
    }

    #[test]
    fn test_triton_walks_the_perimeter() {
        // A worker on the empty perimeter can chain around the whole ring, but may never reach the
        // far side of the middle: the first step into the middle ends the chain.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(E1)
            .with_p2_worker(C1)
            .with_p2_worker(D1)
            .build();

        let destinations = destinations_from(&state, Player::One, A5);

        for square in [A4, A1, C5, E5, E4, B4, B2] {
            assert!(
                destinations.contains_square(square),
                "{} should be reachable",
                square
            );
        }
        // C3 is only reachable via the middle, and the middle is where a chain stops.
        assert!(!destinations.contains_square(C3));
        // Walking the ring all the way back around is legal, and is the only way to stand still.
        assert!(destinations.contains_square(A5));
    }

    #[test]
    fn test_triton_does_not_chain_off_a_middle_space() {
        // Every neighbor of C3 is a middle space, so this worker takes exactly one step.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(C3)
            .with_p2_worker(A1)
            .build();

        let destinations = destinations_from(&state, Player::One, C3);
        assert_eq!(destinations, NEIGHBOR_MAP[C3 as usize]);
    }

    #[test]
    fn test_triton_wins_by_walking_to_a_springboard() {
        // C1 is a level 3 square that no worker starts next to. B1 is a level 2 perimeter space, so
        // the worker can walk A1 -> B1 and climb off it in the same turn.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(A1)
            .with_p2_worker(E5)
            .with_height(A1, 1)
            .with_height(B1, 2)
            .with_height(C1, 3)
            .build();

        let next_states = state.get_next_states_interactive();
        MoveVerifier::new()
            .is_winner(Player::One)
            .with_p1_worker_at(C1)
            .any(&next_states);
    }

    #[test]
    fn test_triton_cannot_climb_off_a_middle_springboard() {
        // Same shape, but the level 2 springboard is in the middle, so the chain ends there and the
        // climb has to wait for next turn.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(B3)
            .with_p2_worker(E5)
            .with_height(C3, 2)
            .with_height(D3, 3)
            .build();

        let next_states = state.get_next_states_interactive();
        MoveVerifier::new()
            .is_winner(Player::One)
            .none(&next_states);
    }

    #[test]
    fn test_triton_may_stop_after_the_first_step() {
        // The chain is optional, so every single-step destination is still offered.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .build();

        let destinations = destinations_from(&state, Player::One, A5);
        for square in NEIGHBOR_MAP[A5 as usize] {
            assert!(
                destinations.contains_square(square),
                "{} should be reachable in one step",
                square
            );
        }
    }

    #[test]
    fn test_triton_chain_respects_climb_limit() {
        // B5 is level 2, so a ground level worker on A5 cannot step onto it, and everything past it
        // along the top row is unreachable.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(B5, 2)
            .with_height(B4, 2)
            .with_height(A4, 2)
            .build();

        let destinations = destinations_from(&state, Player::One, A5);
        assert_eq!(destinations, BitBoard::EMPTY);
    }

    #[test]
    fn test_triton_consistency_on_a_crowded_board() {
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
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
    fn test_triton_vs_persephone_climb_may_come_mid_chain() {
        // A5 -> A4 is flat, A4 -> A3 climbs, A3 -> A2 comes back down to where the turn started.
        // Persephone is satisfied by the climb happening anywhere in the chain, so A2 is legal even
        // though the turn ends no higher than it began.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Persephone)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .with_height(A3, 1)
            .build();

        let destinations = destinations_from(&state, Player::One, A5);
        assert!(
            destinations.contains_square(A3),
            "the climb itself is legal"
        );
        assert!(
            destinations.contains_square(A2),
            "stepping back down past the climb is legal"
        );
    }

    #[test]
    fn test_triton_vs_persephone_drops_chains_that_never_climb() {
        // Every neighbor of C3 is a middle space, so there is no chain to hide a climb in: only the
        // single step onto the level 1 square survives.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Persephone)
            .with_p1_worker(C3)
            .with_p2_worker(E1)
            .with_height(B2, 1)
            .build();

        assert_eq!(
            destinations_from(&state, Player::One, C3),
            BitBoard::as_mask(B2)
        );
    }

    #[test]
    fn test_triton_vs_persephone_falls_back_when_no_climb_exists() {
        let state = GameStateBuilder::new(GodName::Triton, GodName::Persephone)
            .with_p1_worker(A5)
            .with_p2_worker(E1)
            .build();

        assert!(destinations_from(&state, Player::One, A5).is_not_empty());
    }

    #[test]
    fn test_triton_search_finds_the_walking_win() {
        // Exercises the whole search path - move ordering, make/unmake, eval - with a god whose
        // move list contains stand-still moves.
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(A1)
            .with_p1_worker(E5)
            .with_p2_worker(C5)
            .with_p2_worker(E3)
            .with_height(A1, 1)
            .with_height(B1, 2)
            .with_height(C1, 3)
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

    #[test]
    fn test_triton_blocker_board_covers_the_walk() {
        let state = GameStateBuilder::new(GodName::Triton, GodName::Mortal)
            .with_p1_worker(A1)
            .with_p2_worker(E5)
            .with_height(A1, 1)
            .with_height(B1, 2)
            .with_height(C1, 3)
            .build();

        let triton = GodName::Triton.to_power();
        let wins = triton.get_winning_moves(&state, Player::One);
        assert_eq!(wins.len(), 1);

        let blockers = triton.get_blocker_board(&state.board, wins[0].action);
        // B1 is the springboard: building it up to a dome, or standing on it, is what stops this.
        assert!(blockers.contains_square(B1));
        assert!(blockers.contains_square(A1));
        assert!(blockers.contains_square(C1));
    }
}
