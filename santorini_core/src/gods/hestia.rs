use crate::{
    bitboard::{BitBoard, MIDDLE_SPACES_MASK},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_next_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const SECOND_BUILD_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const NO_SECOND_BUILD_VALUE: MoveData = 25 << SECOND_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
struct HestiaMove(pub MoveData);

impl Into<GenericMove> for HestiaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for HestiaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl HestiaMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | NO_SECOND_BUILD_VALUE;

        Self(data)
    }

    fn new_two_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        build_position_2: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((build_position_2 as MoveData) << SECOND_BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn second_build_position(self) -> Option<Square> {
        let value = (self.0 >> SECOND_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for HestiaMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            write!(f, "{}>{}#", move_from, move_to)
        } else if let Some(second_build) = self.second_build_position() {
            write!(f, "{}>{}^{}^{}", move_from, move_to, build, second_build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for HestiaMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut move_vec = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![move_vec];
        }

        let build_position = self.build_position();
        if let Some(second_build) = self.second_build_position() {
            let build_mask = BitBoard::as_mask(build_position);
            if second_build != build_position && (build_mask & MIDDLE_SPACES_MASK).is_not_empty() {
                let mut mirror = move_vec.clone();

                move_vec.push(PartialAction::Build(build_position));
                move_vec.push(PartialAction::Build(second_build));

                mirror.push(PartialAction::Build(second_build));
                mirror.push(PartialAction::Build(build_position));
                vec![move_vec, mirror]
            } else {
                move_vec.push(PartialAction::Build(build_position));
                move_vec.push(PartialAction::Build(second_build));
                vec![move_vec]
            }
        } else {
            move_vec.push(PartialAction::Build(build_position));
            vec![move_vec]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);
        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
        if let Some(build_position) = self.second_build_position() {
            board.build_up(build_position);
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
        helper.add_maybe_square_with_height(board, self.second_build_position());
        helper.get()
    }
}

pub(super) fn hestia_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(hestia_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, HestiaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                HestiaMove::new_winning_move,
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
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            let all_second_builds =
                worker_next_build_state.all_possible_builds & MIDDLE_SPACES_MASK;
            let narrowed_second_builds =
                worker_next_build_state.narrowed_builds & MIDDLE_SPACES_MASK;

            for worker_build_pos in worker_next_build_state.all_possible_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let is_first_middle_build = (worker_build_mask & MIDDLE_SPACES_MASK).is_not_empty();

                let is_first_build_interact =
                    ((worker_build_mask | worker_end_move_state.worker_end_mask) & key_squares)
                        .is_not_empty();

                let second_builds =
                    if is_interact_with_key_squares::<F>() && !is_first_build_interact {
                        narrowed_second_builds
                    } else {
                        all_second_builds
                    };

                for second_worker_build_pos in second_builds {
                    let second_worker_build_mask = BitBoard::as_mask(second_worker_build_pos);
                    if is_first_middle_build && second_worker_build_pos < worker_build_pos {
                        continue;
                    }

                    let is_check;
                    if worker_build_pos == second_worker_build_pos {
                        if prelude.board.height_lookup[worker_build_pos as usize] >= 3 {
                            continue;
                        }

                        is_check = {
                            let final_level_3 = (prelude.exactly_level_1 & worker_build_mask)
                                | (prelude.exactly_level_3 & !worker_build_mask);
                            let check_board = reach_board & final_level_3;
                            check_board.is_not_empty()
                        };
                    } else {
                        let total_build_mask = worker_build_mask | second_worker_build_mask;
                        is_check = {
                            let final_level_3 = (prelude.exactly_level_2 & total_build_mask)
                                | (prelude.exactly_level_3 & !total_build_mask);
                            let check_board = reach_board & final_level_3;
                            check_board.is_not_empty()
                        };
                    }

                    let new_action = HestiaMove::new_two_build_move(
                        worker_start_state.worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                        second_worker_build_pos,
                    );

                    result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        worker_end_move_state.is_improving,
                    ))
                }

                if !is_interact_with_key_squares::<F>() || is_first_build_interact {
                    let new_action = HestiaMove::new_basic_move(
                        worker_start_state.worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                    );

                    let is_check = {
                        let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                            | (prelude.exactly_level_3 & !worker_build_mask);
                        let check_board = reach_board & final_level_3;
                        check_board.is_not_empty()
                    };

                    result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        worker_end_move_state.is_improving,
                    ))
                }
            }
        }
    }

    result
}

pub const fn build_hestia() -> GodPower {
    god_power(
        GodName::Hestia,
        build_god_power_movers!(hestia_move_gen),
        build_god_power_actions::<HestiaMove>(),
        12982186464139786853,
        3782535430861395330,
    )
}
