use crate::{
    add_scored_move,
    bitboard::{BitBoard, NEIGHBOR_MAP, PUSH_MAPPING},
    board::{BoardState, FullGameState, },
    build_god_power_movers, build_parse_flags,
    gods::{
        build_god_power_actions, generic::{
            GenericMove, GodMove, MoveData, MoveGenFlags, ScoredMove, LOWER_POSITION_MASK, MATE_ONLY, MOVE_IS_WINNING_MASK, NULL_MOVE_DATA, POSITION_WIDTH
        }, god_power, FullAction, GodName, GodPower
    },
    player::Player,
    square::Square,
    variable_prelude,
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
        let from = self.move_from_position();
        let to = self.move_to_position();
        let build = self.build_position();

        let from_height = board.get_height(from);
        let to_height = board.get_height(to);
        let build_height = board.get_height(build);

        let fu = from as usize;
        let tu = to as usize;
        let bu = build as usize;

        let mut res = 4 * fu + from_height;
        res = res * 100 + 4 * tu + to_height;
        res = res * 100 + 4 * bu + build_height;
        res = res * 2 + self.push_to_position().is_some() as usize;

        res
    }
}

fn minotaur_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    build_parse_flags!(
        is_mate_only,
        is_include_score,
        is_stop_on_mate,
        is_interact_with_key_squares
    );

    variable_prelude!(
       state:  state,
       player:  player,
       board:  board,
       other_player:  other_player,
       current_player_idx:  current_player_idx,
       other_player_idx:  other_player_idx,
       other_god:  other_god,
       exactly_level_0:  exactly_level_0,
       exactly_level_1:  exactly_level_1,
       exactly_level_2:  exactly_level_2,
       exactly_level_3:  exactly_level_3,
       domes:  domes,
       win_mask:  win_mask,
       build_mask: build_mask,
       is_against_hypnus: is_against_hypnus,
       is_against_harpies: _is_against_harpies,
       own_workers:  own_workers,
       oppo_workers:  oppo_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
       acting_workers:  acting_workers,
       checkable_worker_positions_mask:  checkable_worker_positions_mask,
    );
    let blocked_squares = all_workers_mask | domes;

    for moving_worker_start_pos in acting_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let other_own_workers = own_workers ^ moving_worker_start_mask;

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | own_workers);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & exactly_level_3 & win_mask;
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
                if (moving_worker_end_mask & oppo_workers).is_not_empty() {
                    if let Some(push_to) = PUSH_MAPPING[moving_worker_start_pos as usize]
                        [moving_worker_end_pos as usize]
                    {
                        let push_to_mask = BitBoard::as_mask(push_to);
                        if (push_to_mask & blocked_squares).is_empty() {
                            let winning_move = ScoredMove::new_winning_move(
                                MinotaurMove::new_minotaur_winning_push_move(
                                    moving_worker_start_pos,
                                    moving_worker_end_pos,
                                    push_to,
                                )
                                .into(),
                            );
                            result.push(winning_move);
                            if is_stop_on_mate {
                                return result;
                            }
                        }
                    }
                } else {
                    let winning_move = ScoredMove::new_winning_move(
                        MinotaurMove::new_winning_move(
                            moving_worker_start_pos,
                            moving_worker_end_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate {
                        return result;
                    }
                }
            }
        }

        if is_mate_only {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let all_buildable_squares = !(non_selected_workers | domes);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);
            let is_improving = worker_end_height > worker_starting_height;

            let end_neighbors = NEIGHBOR_MAP[moving_worker_end_pos as usize];
            let mut worker_builds = end_neighbors & all_buildable_squares;
            let mut push_to_spot: Option<Square> = None;
            let mut push_to_mask = BitBoard::EMPTY;

            let mut final_build_mask = build_mask;
            let mut other_workers_post_push = oppo_workers;

            if (moving_worker_end_mask & oppo_workers).is_not_empty() {
                if let Some(push_to) =
                    PUSH_MAPPING[moving_worker_start_pos as usize][moving_worker_end_pos as usize]
                {
                    let tmp_push_to_mask = BitBoard::as_mask(push_to);
                    if (tmp_push_to_mask & blocked_squares).is_empty() {
                        push_to_spot = Some(push_to);
                        push_to_mask = tmp_push_to_mask;
                        worker_builds ^= tmp_push_to_mask;

                        other_workers_post_push =
                            oppo_workers ^ push_to_mask ^ moving_worker_end_mask;
                        final_build_mask =
                            other_god.get_build_mask(other_workers_post_push) | exactly_level_3;
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            worker_builds &= final_build_mask;

            if is_interact_with_key_squares {
                if ((moving_worker_end_mask | push_to_mask) & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let free_move_spaces = !(other_own_workers | domes | moving_worker_end_mask);
            let not_other_pushed_workers = !other_workers_post_push;

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);

                let new_action = if let Some(push_to) = push_to_spot {
                    MinotaurMove::new_minotaur_push_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        worker_build_pos,
                        push_to,
                    )
                } else {
                    MinotaurMove::new_minotaur_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        worker_build_pos,
                    )
                };

                let is_check = {
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);
                    let possible_dest_board = final_level_3 & win_mask & free_move_spaces;
                    let checkable_own_workers =
                        (other_own_workers | moving_worker_end_mask) & exactly_level_2;

                    let mut is_check = false;

                    if !is_against_hypnus || checkable_own_workers.count_ones() >= 2 {
                        let blocked_for_final_push_squares = other_own_workers
                            | moving_worker_end_mask
                            | domes
                            | (exactly_level_3 & worker_build_mask)
                            | other_workers_post_push;

                        for worker in checkable_own_workers {
                            let ns = NEIGHBOR_MAP[worker as usize] & possible_dest_board;
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

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
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
