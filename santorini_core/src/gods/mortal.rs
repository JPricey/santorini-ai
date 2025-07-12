use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            GRID_POSITION_SCORES, GenericMove, INCLUDE_SCORE, LOWER_POSITION_MASK, MATE_ONLY,
            MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags, MoveScore, POSITION_WIDTH,
            RETURN_FIRST_MATE, STOP_ON_MATE, WORKER_HEIGHT_SCORES,
        },
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

// from(5)|to(5)|build(5)|win(1)
pub const MORTAL_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const MORTAL_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const MORTAL_BUILD_POSITION_OFFSET: usize = POSITION_WIDTH * 2;

impl GenericMove {
    pub fn new_mortal_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> GenericMove {
        let data: MoveData = ((move_from_position as MoveData) << MORTAL_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MORTAL_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << MORTAL_BUILD_POSITION_OFFSET);

        Self::new(data)
    }

    pub fn new_mortal_winning_move(
        move_from_position: Square,
        move_to_position: Square,
    ) -> GenericMove {
        let data: MoveData = ((move_from_position as MoveData) << MORTAL_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MORTAL_MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self::new_winning_move(data)
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.data as u8) & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.data >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    pub fn mortal_move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn mortal_build_position(self) -> u8 {
        ((self.data >> MORTAL_BUILD_POSITION_OFFSET) as u8) & LOWER_POSITION_MASK
    }
}

pub fn mortal_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
    let current_player = board.current_player;
    let worker_move_mask = action.mortal_move_mask();
    let current_workers = board.workers[current_player as usize];

    let moving_worker_mask = current_workers & worker_move_mask;
    let result_worker_mask = worker_move_mask ^ moving_worker_mask;

    if action.get_is_winning() {
        return vec![vec![
            PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
            PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        ]];
    }

    let build_position = action.mortal_build_position();
    return vec![vec![
        PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
        PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        PartialAction::Build(Square::from(build_position as usize)),
    ]];
}

pub fn mortal_make_move(board: &mut BoardState, action: GenericMove) {
    let worker_move_mask = action.mortal_move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.set_winner(board.current_player);
        return;
    }

    let build_position = action.mortal_build_position();
    let build_mask = BitBoard::as_mask_u8(build_position);

    let build_height = board.get_height_for_worker(build_mask);
    board.height_map[build_height] |= build_mask;
}

pub fn mortal_unmake_move(board: &mut BoardState, action: GenericMove) {
    let worker_move_mask = action.mortal_move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.unset_winner(board.current_player);
        return;
    }

    let build_position = action.mortal_build_position();
    let build_mask = BitBoard::as_mask_u8(build_position);

    let build_height = board.get_true_height(build_mask);
    board.height_map[build_height - 1] ^= build_mask;
}

fn mortal_move_gen<const F: MoveGenFlags>(board: &BoardState, player: Player) -> Vec<GenericMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= board.exactly_level_2()
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];

    let mut help_oppo_builds = BitBoard::EMPTY;
    let mut hurt_oppo_builds = BitBoard::EMPTY;

    for oppo_pos in board.workers[1 - current_player_idx] {
        let oppo_height = board.get_height_for_worker(BitBoard::as_mask(oppo_pos));
        let ns = NEIGHBOR_MAP[oppo_pos as usize];
        hurt_oppo_builds |= ns & board.height_map[oppo_height];
        help_oppo_builds |= ns & !board.height_map[oppo_height];
    }

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let mut help_self_builds = BitBoard::EMPTY;
        let mut hurt_self_builds = BitBoard::EMPTY;

        let other_self_workers = current_workers ^ moving_worker_start_mask;
        for other_self_pos in other_self_workers {
            let other_height = board.get_height_for_worker(BitBoard::as_mask(other_self_pos));
            let ns = NEIGHBOR_MAP[other_self_pos as usize];
            help_self_builds |= ns & !board.height_map[other_height];
            hurt_self_builds |= ns & board.height_map[other_height];
        }

        let baseline_score: MoveScore = 0
            - GRID_POSITION_SCORES[moving_worker_start_pos as usize]
            - WORKER_HEIGHT_SCORES[worker_starting_height];

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[too_high] | all_workers_mask);

        if F & MATE_ONLY > 0 || worker_starting_height != 3 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = GenericMove::new_mortal_winning_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                );
                result.push(winning_move);
                if F & STOP_ON_MATE != 0 {
                    return result;
                }
            }
        }

        if F & MATE_ONLY != 0 {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let ns = NEIGHBOR_MAP[moving_worker_end_pos as usize];
            let help_self_builds = help_self_builds | ns & !board.height_map[worker_end_height];
            let hurt_self_builds = hurt_self_builds | ns & board.height_map[worker_end_height];

            let baseline_score = baseline_score
                + GRID_POSITION_SCORES[moving_worker_end_pos as usize]
                + WORKER_HEIGHT_SCORES[worker_end_height as usize];

            let worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            let (check_count, builds_that_result_in_checks, build_that_remove_checks) =
                if worker_end_height == 2 {
                    let exactly_level_2 = board.height_map[1] & !board.height_map[2];
                    let level_3 = board.height_map[2];
                    // (worker_builds & exactly_level_2)
                    let check_count = (worker_builds & level_3).count_ones();
                    let builds_that_result_in_checks = worker_builds & exactly_level_2;
                    let builds_that_remove_checks = worker_builds & level_3;
                    (
                        check_count as MoveScore,
                        builds_that_result_in_checks,
                        builds_that_remove_checks,
                    )
                } else {
                    (0, BitBoard::EMPTY, BitBoard::EMPTY)
                };

            for worker_build_pos in worker_builds {
                let mut new_action = GenericMove::new_mortal_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let build_height = board.get_height_for_worker(worker_build_mask);

                    let bh2 = ((build_height + 1) * (build_height + 1)) as MoveScore;

                    let build_scores = 0
                        + (help_self_builds & worker_build_mask).count_ones() as MoveScore
                            * 9
                            * (build_height + 1) as MoveScore
                        - bh2 * (hurt_self_builds & worker_build_mask).count_ones() as MoveScore
                        + bh2
                            * (hurt_oppo_builds & worker_build_mask).count_ones() as MoveScore
                            * 2
                        - (help_oppo_builds & worker_build_mask).count_ones() as MoveScore
                            * 11
                            * (build_height + 1) as MoveScore;

                    let check_count = check_count
                        + ((builds_that_result_in_checks & BitBoard::as_mask(worker_build_pos))
                            .is_not_empty() as MoveScore)
                        - ((build_that_remove_checks & BitBoard::as_mask(worker_build_pos))
                            .is_not_empty() as MoveScore);

                    // new_action.set_score(baseline_score + build_height as MoveScore);
                    new_action.set_score(baseline_score + build_scores * 20 + check_count * 2500);
                }
                result.push(new_action);
            }
        }
    }

    result
}

pub const fn build_mortal() -> GodPower {
    GodPower {
        god_name: GodName::Mortal,
        get_all_moves: mortal_move_gen::<0>,
        get_moves: mortal_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        get_win: mortal_move_gen::<{ RETURN_FIRST_MATE }>,
        get_actions_for_move: mortal_move_to_actions,
        _make_move: mortal_make_move,
        _unmake_move: mortal_unmake_move,
    }
}

/*
#[cfg(test)]
mod tests {
    use crate::{board::FullGameState, gods::tests::assert_has_win_consistency, player::Player};

    #[test]
    fn test_mortal_win_checking() {
        {
            let state_str = "00000 00000 00230 00000 00030/1/mortal:12/mortal:24";
            let mut state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, true);
            state.board.current_player = Player::Two;
            assert_has_win_consistency(&state, false);
        }

        {
            // level 3 is next, but it's blocked by a worker
            let state_str = "00000 00000 00230 00000 00030/1/mortal:12/mortal:13";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, false);
        }

        {
            // level 3 is next, but you're already on level 3
            let state_str = "00000 00000 00330 00000 00030/1/mortal:12/mortal:24";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, false);
        }

        {
            let state_str = "2300000000000000000000000/2/mortal:2,13/mortal:0,17";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, true);
        }

        {
            let state_str = "2144330422342221044000400/2/mortal:1,13/mortal:8,9";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, true);
        }
    }
}
*/
