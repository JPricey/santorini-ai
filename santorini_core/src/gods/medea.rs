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
        mortal::mortal_move_gen,
        move_helpers::{
            GeneratorPreludeState, WorkerEndMoveState, WorkerNextMoveState, WorkerStartMoveState,
            build_scored_move, get_generator_prelude_state,
            get_standard_reach_board_from_parts, get_worker_end_move_state,
            get_worker_next_build_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

/// Medea takes an ordinary mortal turn, and once in the game she may finish it by razing a square:
/// every block under some worker standing next to one of hers comes off, dropping that worker to
/// the ground.
///
/// The power resolves *after* the move and the build, so the neighbours it reaches are the ones
/// standing there at the end of the turn - including the worker that just moved, and including her
/// own workers when they neighbour each other. Since a build lands on an empty square and a raze
/// needs an occupied one, the two never name the same square, which is what spares this god the
/// "did the removal undo my own build" case Ares has to carry.
///
/// The from/to/build fields sit exactly where `MortalMove` puts them and the razed square is packed
/// above them as `square + 1`, so that an all-zero field means "no raze". Razing is the only thing
/// that spends the power, so that field doubles as the USE_POWER discriminant, and a non-razing
/// move is bit-identical to the equivalent mortal move. That is what lets the generator hand the
/// whole turn to `mortal_move_gen` once the power is spent.
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const RAZE_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

const _LAYOUT_ASSERT: () =
    assert!(((1 as MoveData) << (RAZE_POSITION_OFFSET + POSITION_WIDTH)) - 1 & !MOVE_DATA_MAIN_SECTION == 0);
const _MORTAL_LAYOUT_ASSERT: () = {
    assert!(MOVE_FROM_POSITION_OFFSET == 0);
    assert!(MOVE_TO_POSITION_OFFSET == POSITION_WIDTH);
    assert!(BUILD_POSITION_OFFSET == 2 * POSITION_WIDTH);
};

/// Power available / power spent, mirroring the other once-per-game gods.
const POWER_AVAILABLE: GodData = 0;
const POWER_SPENT: GodData = 1;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MedeaMove(pub MoveData);

impl Into<GenericMove> for MedeaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for MedeaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl MedeaMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_raze_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        raze_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | (((raze_position as MoveData) + 1) << RAZE_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
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

    pub fn raze_position(self) -> Option<Square> {
        let value = (self.0 >> RAZE_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 0 {
            None
        } else {
            Some(Square::from(value - 1))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for MedeaMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();

        if self.get_is_winning() {
            return write!(f, "{}>{}#", move_from, move_to);
        }

        let build = self.build_position();
        if let Some(raze) = self.raze_position() {
            write!(f, "{}>{}^{}~~{}", move_from, move_to, build, raze)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for MedeaMove {
    fn move_to_actions(
        self,
        board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));

        if let Some(raze) = self.raze_position() {
            // The whole tower comes off at once, but the only vocabulary the UI has for that is
            // "remove a block", so say it once per block. Replaying the actions then lands on the
            // same board `make_move` produces. The raze never targets the square just built on -
            // builds land on empty squares and the raze needs a worker - so the pre-move height is
            // the right count.
            for _ in 0..board.get_height(raze) {
                res.push(PartialAction::Destroy(raze));
            }
        }

        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        board.worker_xor(player, self.move_mask());

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());

        if let Some(raze) = self.raze_position() {
            board.set_god_data(player, POWER_SPENT);
            board.raze(raze);
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_maybe_square_with_height(board, self.raze_position());
        helper.get()
    }
}

/// The workers she may strip, given where hers stand at the end of the turn.
///
/// Adjacency here is the power's own, not the movement rules', so it uses the plain neighbour map
/// the way Ares' removal does - an opposing Aeolus or Hippolyta bends where workers may *walk*, not
/// who stands next to whom. Ground level squares are left out: razing one is a no-op that spends
/// the power for nothing, so those moves would only ever be dominated. Frozen squares (Clio's
/// coins, Europa's Talus) are off limits, since their owner treats them as domes.
fn _raze_targets(prelude: &GeneratorPreludeState, own_workers: BitBoard) -> BitBoard {
    apply_mapping_to_mask(own_workers, &NEIGHBOR_MAP)
        & (own_workers | prelude.oppo_workers)
        & prelude.board.height_map[0]
        & !prelude.domes_and_frozen
}

pub(super) fn medea_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if state.board.god_data[player as usize] != POWER_AVAILABLE {
        return mortal_move_gen::<F, MUST_CLIMB>(state, player, key_squares);
    }

    let mut result = persephone_check_result!(medea_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            // Razing only ever takes blocks away, so it can neither create a win nor rescue one.
            // Wins are plain mortal climbs, with the power left in hand.
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, MedeaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                MedeaMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let own_end_workers =
                worker_start_state.other_own_workers | worker_end_move_state.worker_end_mask;
            let raze_targets = _raze_targets(&prelude, own_end_workers);

            let reach_board = get_standard_reach_board_from_parts::<F>(
                &prelude,
                worker_next_moves.other_threatening_workers,
                worker_next_moves.other_threatening_neighbors,
                worker_end_move_state.worker_end_pos,
                worker_end_move_state.is_now_lvl_2,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                    | (prelude.exactly_level_3 & !worker_build_mask);

                let new_action = MedeaMove::new_basic_move(
                    worker_start_state.worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                result.push(build_scored_move::<F, _>(
                    new_action,
                    (reach_board & final_level_3).is_not_empty(),
                    worker_end_move_state.is_improving,
                ));

                for raze_pos in raze_targets {
                    result.push(_build_raze_move::<F>(
                        &prelude,
                        &worker_start_state,
                        &worker_end_move_state,
                        &worker_next_moves,
                        worker_next_build_state.unblocked_squares,
                        worker_build_pos,
                        final_level_3,
                        raze_pos,
                    ));
                }
            }

            if is_interact_with_key_squares::<F>() {
                // A raze is a block in its own right: stripping the tower under an opposing worker
                // takes away whatever climb it was about to make. Those moves are invisible to the
                // narrowing above, which only looks at where the worker lands and where it builds.
                let raze_blockers = raze_targets & key_squares;
                if raze_blockers.is_empty() {
                    continue;
                }
                let non_narrowed_builds = worker_next_build_state.all_possible_builds
                    & !worker_next_build_state.narrowed_builds;

                for worker_build_pos in non_narrowed_builds {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask);

                    for raze_pos in raze_blockers {
                        result.push(_build_raze_move::<F>(
                            &prelude,
                            &worker_start_state,
                            &worker_end_move_state,
                            &worker_next_moves,
                            worker_next_build_state.unblocked_squares,
                            worker_build_pos,
                            final_level_3,
                            raze_pos,
                        ));
                    }
                }
            }
        }
    }

    result
}

/// Scores one move that ends in a raze.
///
/// A raze can only ever cancel threats, never create one, but it can cancel her own: the razed
/// square drops out of the level 3 set, and if it was under one of her own workers that worker is
/// no longer standing where it was, so it stops threatening anything. Both have to be folded in,
/// because the consistency checker compares the check flag against ground truth in both directions.
#[inline]
fn _build_raze_move<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    worker_end_move_state: &WorkerEndMoveState,
    worker_next_moves: &WorkerNextMoveState,
    unblocked_squares: BitBoard,
    worker_build_pos: Square,
    final_level_3: BitBoard,
    raze_pos: Square,
) -> ScoredMove {
    let raze_mask = BitBoard::as_mask(raze_pos);

    // Razing her own worker's square drops it to the ground, so it threatens nothing next turn -
    // whether it is the worker that just moved or the one that stayed put. Razing anyone else's
    // leaves her reach alone.
    let stays_on_lvl_2 = worker_end_move_state.is_now_lvl_2
        * (raze_pos != worker_end_move_state.worker_end_pos) as u32;
    let threatening_workers = worker_next_moves.other_threatening_workers & !raze_mask;
    let threatening_neighbors =
        if threatening_workers == worker_next_moves.other_threatening_workers {
            worker_next_moves.other_threatening_neighbors
        } else {
            apply_mapping_to_mask(threatening_workers, prelude.standard_neighbor_map)
        };

    let reach_board = get_standard_reach_board_from_parts::<F>(
        prelude,
        threatening_workers,
        threatening_neighbors,
        worker_end_move_state.worker_end_pos,
        stays_on_lvl_2,
        unblocked_squares,
    );

    let new_action = MedeaMove::new_raze_move(
        worker_start_state.worker_start_pos,
        worker_end_move_state.worker_end_pos,
        worker_build_pos,
        raze_pos,
    );

    let is_check = (reach_board & final_level_3 & !raze_mask).is_not_empty();
    let is_improving = worker_end_move_state.is_improving
        && raze_pos != worker_end_move_state.worker_end_pos;

    build_scored_move::<F, _>(new_action, is_check, is_improving)
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(POWER_AVAILABLE),
        "x" | "X" => Ok(POWER_SPENT),
        _ => Err(format!("Must be either empty string or x")),
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        POWER_AVAILABLE => None,
        _ => Some(format!("x")),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        POWER_AVAILABLE => Some(format!("Power available")),
        _ => Some(format!("Power used")),
    }
}

/// She borrows Bellerophon's NNUE weights: also a mortal carrying a single once-per-game flag, with
/// `god_data` meaning the same thing in both (0 available, 1 spent), so the "power spent" input
/// carries over rather than being dropped the way a Mortal proxy would drop it.
pub const fn build_medea() -> GodPower {
    god_power(
        GodName::Medea,
        build_god_power_movers!(medea_move_gen),
        build_god_power_actions::<MedeaMove>(),
        2739103548185174811,
        15600432297185516803,
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
        consistency_checker::consistency_check,
        fen::parse_fen,
        move_verifier::MoveVerifier,
        pretty_board::game_state_with_partial_actions,
        square::Square::*,
    };

    use super::*;

    /// Every square Medea is offered as a raze target for the given worker move.
    fn raze_targets_for(state: &FullGameState, player: Player, from: Square, to: Square) -> BitBoard {
        let mut res = BitBoard::EMPTY;
        for scored in GodName::Medea.to_power().get_all_moves(state, player) {
            let action: MedeaMove = scored.action.into();
            if action.move_from_position() != from || action.move_to_position() != to {
                continue;
            }
            if let Some(raze) = action.raze_position() {
                res |= BitBoard::as_mask(raze);
            }
        }
        res
    }

    #[test]
    fn test_medea_move_fits_in_the_main_section() {
        let plain = MedeaMove::new_basic_move(A1, B2, C3);
        assert_eq!(plain.move_from_position(), A1);
        assert_eq!(plain.move_to_position(), B2);
        assert_eq!(plain.build_position(), C3);
        assert_eq!(plain.raze_position(), None);
        assert_eq!(plain.0 & MOVE_DATA_MAIN_SECTION, plain.0);

        // A5 is square 0, so it is the one that would be mistaken for "no raze" without the +1.
        for raze in [A5, A1, E5, E1, C3] {
            let action = MedeaMove::new_raze_move(A1, B2, C3, raze);
            assert_eq!(action.move_from_position(), A1);
            assert_eq!(action.move_to_position(), B2);
            assert_eq!(action.build_position(), C3);
            assert_eq!(action.raze_position(), Some(raze));
            assert_eq!(action.0 & MOVE_DATA_MAIN_SECTION, action.0);
        }
    }

    #[test]
    fn test_medea_strips_a_neighbouring_opponent_to_the_ground() {
        // D5 is a level 3 square with an opponent worker parked on it, next to Medea's C5.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(C5)
            .with_p1_worker(A1)
            .with_p2_worker(D5)
            .with_p2_worker(E1)
            .with_height(D5, 3)
            .build();

        let next_states = state.get_next_states_interactive();
        MoveVerifier::new()
            .with_p2_worker_at(D5)
            .with_height_at(D5, 0)
            .any(&next_states);
    }

    #[test]
    fn test_medea_can_raze_her_own_worker() {
        // Her two workers neighbour each other, so B5 is a legal target for the power - the card
        // says any worker neighbouring either of hers, and hers neighbour each other.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(B5)
            .with_p2_worker(E1)
            .with_p2_worker(E2)
            .with_height(B5, 2)
            .build();

        assert!(raze_targets_for(&state, Player::One, A5, A4).contains_square(B5));
    }

    #[test]
    fn test_medea_can_raze_the_square_she_just_climbed_onto() {
        // The power resolves at the end of the turn, so the worker that moved is standing next to
        // her other worker by then and is fair game itself.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(C5)
            .with_p2_worker(E1)
            .with_p2_worker(E2)
            .with_height(B5, 1)
            .build();

        assert!(raze_targets_for(&state, Player::One, A5, B5).contains_square(B5));
    }

    #[test]
    fn test_medea_does_not_raze_out_of_reach_or_empty_squares() {
        // C1 holds an opponent worker but neighbours neither of her workers; D5 is a level 3
        // square with nobody on it; B5 is occupied but already at ground level, so razing it
        // would spend the power for nothing.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(A1)
            .with_p2_worker(B5)
            .with_p2_worker(C1)
            .with_height(D5, 3)
            .build();

        let targets = raze_targets_for(&state, Player::One, A5, A4);
        assert!(!targets.contains_square(C1), "out of reach");
        assert!(!targets.contains_square(D5), "nobody is standing there");
        assert!(!targets.contains_square(B5), "already on the ground");
        assert!(targets.is_empty());
    }

    #[test]
    fn test_medea_spends_the_power_only_when_she_razes() {
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(A1)
            .with_p2_worker(B5)
            .with_p2_worker(E1)
            .with_height(B5, 2)
            .build();

        let medea = GodName::Medea.to_power();
        let mortal = GodName::Mortal.to_power();

        let mut saw_raze = false;
        for scored in medea.get_all_moves(&state, Player::One) {
            let action: MedeaMove = scored.action.into();
            let next = state.next_state(medea, mortal, scored.action);
            let expected = if action.raze_position().is_some() {
                saw_raze = true;
                POWER_SPENT
            } else {
                POWER_AVAILABLE
            };
            assert_eq!(
                next.board.god_data[Player::One as usize], expected,
                "{:?} should leave god data {}",
                action, expected
            );
        }
        assert!(saw_raze);
    }

    #[test]
    fn test_medea_power_is_spent_for_good() {
        let state =
            parse_fen("02000 00000 00000 00000 00000/1/medea[x]:A5,A1/mortal:B5,E1").unwrap();
        assert_eq!(raze_targets_for(&state, Player::One, A5, A4), BitBoard::EMPTY);

        // ...and once spent she reads exactly like a mortal.
        let as_mortal =
            parse_fen("02000 00000 00000 00000 00000/1/mortal:A5,A1/mortal:B5,E1").unwrap();
        assert_eq!(
            GodName::Medea
                .to_power()
                .get_all_moves(&state, Player::One)
                .len(),
            GodName::Mortal
                .to_power()
                .get_all_moves(&as_mortal, Player::One)
                .len()
        );
    }

    #[test]
    fn test_medea_wins_without_paying_for_it() {
        // A plain climb to level 3, with an opponent worker sitting next to her the whole time.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(E5)
            .with_p2_worker(B4)
            .with_p2_worker(E1)
            .with_height(A5, 2)
            .with_height(B5, 3)
            .with_height(B4, 2)
            .build();

        let wins = GodName::Medea
            .to_power()
            .get_winning_moves(&state, Player::One);
        assert_eq!(wins.len(), 1);

        let win: MedeaMove = wins[0].action.into();
        assert_eq!(win.move_to_position(), B5);
        assert_eq!(win.raze_position(), None, "a win has no use for the power");
    }

    #[test]
    fn test_medea_razes_the_opponent_off_his_springboard() {
        // Black stands on C3 at level 2 with a level 3 square at C2: a win next turn. Medea cannot
        // build on C2 to stop it - it is already level 3 - but she can pull the tower out from
        // under C3, which drops him to the ground and takes the climb away.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(B3)
            .with_p1_worker(A1)
            .with_p2_worker(C3)
            .with_p2_worker(E5)
            .with_current_player(Player::One)
            .with_height(C3, 2)
            .with_height(C2, 3)
            .build();

        let medea = GodName::Medea.to_power();
        let mortal = GodName::Mortal.to_power();

        let mut key_squares = BitBoard::EMPTY;
        for win in mortal.get_winning_moves(&state, Player::Two) {
            key_squares |= mortal.get_blocker_board(&state.board, win.action);
        }

        let blockers = medea.get_scored_blocker_moves(&state, Player::One, key_squares);
        let mut found = false;
        for scored in &blockers {
            let action: MedeaMove = scored.action.into();
            if action.raze_position() != Some(C3) {
                continue;
            }
            found = true;
            let next = state.next_state(medea, mortal, scored.action);
            assert!(
                mortal.get_winning_moves(&next, Player::Two).is_empty(),
                "{:?} should have taken the win away",
                action
            );
        }
        assert!(found, "razing the springboard is a blocking move");
    }

    #[test]
    fn test_medea_partial_actions_replay_to_the_same_board() {
        // The UI previews a turn by replaying its actions, and the only vocabulary it has for the
        // raze is "remove a block" repeated. Check that adds up to the same board `make_move`
        // reaches, from a position where the razed tower is three blocks high.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Apollo)
            .with_p1_worker(C4)
            .with_p1_worker(A1)
            .with_p2_worker(C3)
            .with_p2_worker(E1)
            .with_height(C3, 3)
            .build();

        let medea = GodName::Medea.to_power();
        let apollo = GodName::Apollo.to_power();

        let mut saw_raze = false;
        for scored in medea.get_all_moves(&state, Player::One) {
            let action: MedeaMove = scored.action.into();
            if action.raze_position() != Some(C3) {
                continue;
            }
            saw_raze = true;

            let expected = state.next_state(medea, apollo, scored.action);
            for path in medea.get_actions_for_move(&state.board, scored.action, Player::One, apollo)
            {
                let replayed = game_state_with_partial_actions(&state, &path);
                assert_eq!(
                    replayed.board.height_map, expected.board.height_map,
                    "{:?} replayed to a different board",
                    action
                );
            }
        }
        assert!(saw_raze);
    }

    #[test]
    fn test_medea_check_flag_accounts_for_the_raze() {
        // She lands on B4 at level 2 next to the level 3 square B3, which is a check - unless she
        // then razes B3's neighbourhood out from under herself.
        let state = GameStateBuilder::new(GodName::Medea, GodName::Mortal)
            .with_p1_worker(A4)
            .with_p1_worker(C5)
            .with_p2_worker(E1)
            .with_p2_worker(E2)
            .with_height(A4, 1)
            .with_height(B4, 2)
            .with_height(B3, 3)
            .build();

        assert_eq!(consistency_check(&state), Ok(()));

        let mut saw_plain_check = false;
        let mut saw_self_raze = false;
        for scored in GodName::Medea.to_power().get_moves_for_search(&state, Player::One) {
            let action: MedeaMove = scored.action.into();
            if action.move_from_position() != A4
                || action.move_to_position() != B4
                || action.build_position() != A5
            {
                continue;
            }
            match action.raze_position() {
                None => {
                    saw_plain_check = true;
                    assert!(scored.action.get_is_check(), "{:?}", action);
                }
                Some(B4) => {
                    saw_self_raze = true;
                    assert!(
                        !scored.action.get_is_check(),
                        "{:?} razes the ground she was going to climb from",
                        action
                    );
                }
                Some(_) => {}
            }
        }
        assert!(saw_plain_check && saw_self_raze);
    }
}
