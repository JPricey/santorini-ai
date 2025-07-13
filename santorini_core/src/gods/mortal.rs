use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            ENEMY_WORKER_BUILD_SCORES, GRID_POSITION_SCORES, GenericMove,
            IMPROVER_BUILD_HEIGHT_SCORES, IMPROVER_SENTINEL_SCORE, INCLUDE_SCORE,
            LOWER_POSITION_MASK, MATE_ONLY, MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags,
            MoveScore, NON_IMPROVER_SENTINEL_SCORE, POSITION_WIDTH, RETURN_FIRST_MATE,
            STOP_ON_MATE, WORKER_HEIGHT_SCORES,
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

    pub fn mortal_build_position(self) -> Square {
        Square::from((self.data >> MORTAL_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn mortal_move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
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
    let build_mask = BitBoard::as_mask(build_position);

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
    let build_mask = BitBoard::as_mask(build_position);

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

            let worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            for worker_build_pos in worker_builds {
                let mut new_action = GenericMove::new_mortal_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let is_improving = worker_end_height > worker_starting_height;
                    let score = if is_improving {
                        IMPROVER_SENTINEL_SCORE
                    } else {
                        NON_IMPROVER_SENTINEL_SCORE
                    };
                    new_action.set_score(score);
                }
                result.push(new_action);
            }
        }
    }

    result
}

fn mortal_score_moves<const IMPROVERS_ONLY: bool>(
    board: &BoardState,
    move_list: &mut [GenericMove],
) {
    let mut build_score_map: [MoveScore; 25] = [0; 25];
    for enemy_worker_pos in board.workers[1 - board.current_player as usize] {
        let enemy_worker_height = board.get_height_for_worker(BitBoard::as_mask(enemy_worker_pos));
        let ns = NEIGHBOR_MAP[enemy_worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height_for_worker(BitBoard::as_mask(n_pos));
            build_score_map[n_pos as usize] +=
                2 * ENEMY_WORKER_BUILD_SCORES[enemy_worker_height as usize][n_height as usize];
        }
    }

    for worker_pos in board.workers[board.current_player as usize] {
        let worker_height = board.get_height_for_worker(BitBoard::as_mask(worker_pos));
        let ns = NEIGHBOR_MAP[worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height_for_worker(BitBoard::as_mask(n_pos));
            build_score_map[n_pos as usize] -=
                ENEMY_WORKER_BUILD_SCORES[worker_height as usize][n_height as usize] / 4;
        }
    }

    for action in move_list {
        if IMPROVERS_ONLY && action.score != IMPROVER_SENTINEL_SCORE {
            continue;
        }

        let mut score: MoveScore = 0;

        let from = action.move_from_position();
        let from_height = board.get_height_for_worker(BitBoard::as_mask(from));
        let to = action.move_to_position();
        let to_height = board.get_height_for_worker(BitBoard::as_mask(to));

        let build_at = action.mortal_build_position();
        let build_pre_height = board.get_true_height(BitBoard::as_mask(build_at));

        score -= GRID_POSITION_SCORES[from as usize];
        score += GRID_POSITION_SCORES[to as usize];
        score -= WORKER_HEIGHT_SCORES[from_height as usize] / 3;
        score += WORKER_HEIGHT_SCORES[to_height as usize] / 3;

        score += build_score_map[build_at as usize];

        if IMPROVERS_ONLY {
            score += IMPROVER_BUILD_HEIGHT_SCORES[to_height][build_pre_height];
        }

        action.set_score(score);
    }
}

pub const fn build_mortal() -> GodPower {
    GodPower {
        god_name: GodName::Mortal,
        get_all_moves: mortal_move_gen::<0>,
        _get_moves: mortal_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        get_win: mortal_move_gen::<{ RETURN_FIRST_MATE }>,
        get_actions_for_move: mortal_move_to_actions,
        _score_improvers: mortal_score_moves::<true>,
        _score_remaining: mortal_score_moves::<false>,
        _make_move: mortal_make_move,
        _unmake_move: mortal_unmake_move,
    }
}
