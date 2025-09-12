use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, PUSH_MAPPING, WIND_AWARE_NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::slide_position,
        move_helpers::{
            build_scored_move, get_basic_moves_from_raw_data_with_custom_blockers,
            get_generator_prelude_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

pub const MINOTAUR_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const MINOTAUR_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const MINOTAUR_BUILD_POSITION_OFFSET: usize = MINOTAUR_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const MINOTAUR_PUSH_TO_POSITION_OFFSET: usize = MINOTAUR_BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MinotaurMove(pub MoveData);

impl Into<GenericMove> for MinotaurMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for MinotaurMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl MinotaurMove {
    pub fn new_minotaur_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << MINOTAUR_BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_minotaur_push_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        push_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << MINOTAUR_BUILD_POSITION_OFFSET)
            | ((push_to_position as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((25 as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn new_minotaur_winning_push_move(
        move_from_position: Square,
        move_to_position: Square,
        push_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((push_to_position as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    pub fn push_to_position(&self) -> Option<Square> {
        let value = (self.0 >> MINOTAUR_PUSH_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;

        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn build_position(self) -> Square {
        Square::from((self.0 >> MINOTAUR_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for MinotaurMove {
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
        } else if let Some(push_to) = self.push_to_position() {
            write!(f, "{}>{}(>{})^{}", move_from, move_to, push_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for MinotaurMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut result = vec![PartialAction::SelectWorker(self.move_from_position())];

        if let Some(push_to) = self.push_to_position() {
            result.push(PartialAction::MoveWorkerWithPush(
                self.move_to_position(),
                push_to,
            ));
        } else {
            result.push(PartialAction::MoveWorker(self.move_to_position()));
        }

        if !self.get_is_winning() {
            result.push(PartialAction::Build(self.build_position()));
        }

        return vec![result];
    }

    fn make_move(self, board: &mut BoardState) {
        let move_from = BitBoard::as_mask(self.move_from_position());
        let move_to = BitBoard::as_mask(self.move_to_position());
        board.worker_xor(board.current_player, move_to | move_from);

        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);

        if let Some(push_to) = self.push_to_position() {
            let push_mask = BitBoard::as_mask(push_to);
            board.worker_xor(!board.current_player, move_to | push_mask);
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        let mut result = self.move_mask();

        if let Some(push_pos) = self.push_to_position() {
            result |= BitBoard::as_mask(push_pos);
        }

        result
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_bool(self.push_to_position().is_some());
        helper.get()
    }
}

pub(super) fn minotaur_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(minotaur_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);
    let wind_neighbor_map = &WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx];

    let blocked_squares = prelude.all_workers_mask | prelude.domes;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut worker_moves = get_basic_moves_from_raw_data_with_custom_blockers::<MUST_CLIMB>(
            &prelude,
            worker_start_state.worker_start_pos,
            worker_start_state.worker_start_mask,
            worker_start_state.worker_start_height,
            worker_start_state.other_own_workers,
        );

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;

            for worker_end_pos in moves_to_level_3.into_iter() {
                let moving_worker_end_mask = BitBoard::as_mask(worker_end_pos);
                if (moving_worker_end_mask & prelude.oppo_workers).is_not_empty() {
                    if let Some(push_to) = PUSH_MAPPING
                        [worker_start_state.worker_start_pos as usize]
                        [worker_end_pos as usize]
                    {
                        let push_to_mask = BitBoard::as_mask(push_to);
                        if (push_to_mask & blocked_squares).is_empty() {
                            let winning_move = ScoredMove::new_winning_move(
                                MinotaurMove::new_minotaur_winning_push_move(
                                    worker_start_state.worker_start_pos,
                                    worker_end_pos,
                                    push_to,
                                )
                                .into(),
                            );
                            result.push(winning_move);
                            if is_stop_on_mate::<F>() {
                                return result;
                            }
                        }
                    }
                } else {
                    let winning_move = ScoredMove::new_winning_move(
                        MinotaurMove::new_winning_move(
                            worker_start_state.worker_start_pos,
                            worker_end_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }
            }

            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for mut worker_end_pos in worker_moves {
            let mut worker_end_mask = BitBoard::as_mask(worker_end_pos);

            let mut push_to_spot: Option<Square> = None;
            let mut push_to_mask = BitBoard::EMPTY;

            let mut final_build_mask = prelude.build_mask;
            let mut other_workers_post_push = prelude.oppo_workers;

            if (worker_end_mask & prelude.oppo_workers).is_not_empty() {
                if let Some(push_to) = PUSH_MAPPING[worker_start_state.worker_start_pos as usize]
                    [worker_end_pos as usize]
                {
                    let tmp_push_to_mask = BitBoard::as_mask(push_to);
                    if (tmp_push_to_mask & blocked_squares).is_empty() {
                        push_to_spot = Some(push_to);
                        push_to_mask = tmp_push_to_mask;

                        other_workers_post_push =
                            prelude.oppo_workers ^ push_to_mask ^ worker_end_mask;
                        final_build_mask =
                            prelude.other_god.get_build_mask(other_workers_post_push)
                                | prelude.exactly_level_3;
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if prelude.is_against_harpies && push_to_spot.is_none() {
                worker_end_pos = slide_position(
                    &prelude.board,
                    worker_start_state.worker_start_pos,
                    worker_end_pos,
                );
                worker_end_mask = BitBoard::as_mask(worker_end_pos);
            }

            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_state.worker_start_height;

            let mut worker_builds = NEIGHBOR_MAP[worker_end_pos as usize]
                & !(push_to_mask | worker_start_state.all_non_moving_workers | prelude.domes);
            worker_builds &= final_build_mask;

            if is_interact_with_key_squares::<F>() {
                if ((worker_end_mask | push_to_mask) & key_squares).is_empty() {
                    worker_builds &= key_squares;
                }
            }

            let free_move_spaces =
                !(worker_start_state.other_own_workers | prelude.domes | worker_end_mask);
            let not_other_pushed_workers = !other_workers_post_push;

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);

                let new_action = if let Some(push_to) = push_to_spot {
                    MinotaurMove::new_minotaur_push_move(
                        worker_start_state.worker_start_pos,
                        worker_end_pos,
                        worker_build_pos,
                        push_to,
                    )
                } else {
                    MinotaurMove::new_minotaur_move(
                        worker_start_state.worker_start_pos,
                        worker_end_pos,
                        worker_build_pos,
                    )
                };

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask);
                    let possible_dest_board = final_level_3 & prelude.win_mask & free_move_spaces;
                    let checkable_own_workers = (worker_start_state.other_own_workers
                        | worker_end_mask)
                        & prelude.exactly_level_2;

                    let mut is_check = false;

                    if !prelude.is_against_hypnus || checkable_own_workers.count_ones() >= 2 {
                        let blocked_for_final_push_squares = worker_start_state.other_own_workers
                            | worker_end_mask
                            | prelude.domes
                            | (prelude.exactly_level_3 & worker_build_mask)
                            | other_workers_post_push;

                        for worker in checkable_own_workers {
                            let ns = wind_neighbor_map[worker as usize] & possible_dest_board;
                            if (ns & not_other_pushed_workers).is_not_empty() {
                                is_check = true;
                                break;
                            } else {
                                for o in ns & other_workers_post_push {
                                    if let Some(push_to) = PUSH_MAPPING[worker as usize][o as usize]
                                    {
                                        let tmp_push_to_mask = BitBoard::as_mask(push_to);
                                        if (tmp_push_to_mask & blocked_for_final_push_squares)
                                            .is_empty()
                                        {
                                            is_check = true;
                                            break;
                                        }
                                    }
                                }
                                if is_check {
                                    break;
                                }
                            }
                        }
                    }

                    is_check
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    is_improving,
                ))
            }
        }
    }

    result
}

pub const fn build_minotaur() -> GodPower {
    god_power(
        GodName::Minotaur,
        build_god_power_movers!(minotaur_move_gen),
        build_god_power_actions::<MinotaurMove>(),
        16532879311019593353,
        196173323035994051,
    )
}
