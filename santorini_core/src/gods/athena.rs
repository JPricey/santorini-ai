use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            CHECK_MOVE_BONUS, CHECK_SENTINEL_SCORE, ENEMY_WORKER_BUILD_SCORES,
            GENERATE_THREATS_ONLY, GRID_POSITION_SCORES, GenericMove, IMPROVER_BUILD_HEIGHT_SCORES,
            IMPROVER_SENTINEL_SCORE, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, LOWER_POSITION_MASK,
            MATE_ONLY, MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags, MoveScore,
            NON_IMPROVER_SENTINEL_SCORE, NULL_MOVE_DATA, POSITION_WIDTH, STOP_ON_MATE, ScoredMove,
            WORKER_HEIGHT_SCORES,
        },
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

// from(5)|to(5)|build(5)|win(1)
pub const ATHENA_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const ATHENA_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const ATHENA_BUILD_POSITION_OFFSET: usize = ATHENA_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const ATHENA_DID_IMPROVE_OFFSET: usize = ATHENA_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const ATHENA_DID_IMPROVE_CHANGE_OFFSET: usize = ATHENA_DID_IMPROVE_OFFSET + 1;

pub const ATHENA_DID_IMPROVE_MASK: MoveData = 1 << ATHENA_DID_IMPROVE_OFFSET;
pub const ATHENA_DID_IMPROVE_CHANGE_MASK: MoveData = 1 << ATHENA_DID_IMPROVE_CHANGE_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AthenaMove(pub MoveData);

impl Into<GenericMove> for AthenaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AthenaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AthenaMove {
    pub fn new_athena_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        did_climb: bool,
        did_climb_change: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATHENA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATHENA_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ATHENA_BUILD_POSITION_OFFSET)
            | ((did_climb) as MoveData) << ATHENA_DID_IMPROVE_OFFSET
            | ((did_climb_change) as MoveData) << ATHENA_DID_IMPROVE_CHANGE_OFFSET;

        Self(data)
    }

    pub fn new_athena_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATHENA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATHENA_MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position(self) -> Square {
        Square::from((self.0 >> ATHENA_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }

    pub fn get_did_climb(&self) -> bool {
        (self.0 & ATHENA_DID_IMPROVE_MASK) != 0
    }

    pub fn get_did_climb_change(&self) -> bool {
        (self.0 & ATHENA_DID_IMPROVE_CHANGE_MASK) != 0
    }
}

impl std::fmt::Debug for AthenaMove {
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
        } else if self.get_did_climb() {
            write!(f, "{}>{}!^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

type GodMove = AthenaMove;

pub fn athena_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
    let action: GodMove = action.into();
    let current_player = board.current_player;
    let worker_move_mask = action.move_mask();
    let current_workers = board.workers[current_player as usize];

    let moving_worker_mask = current_workers & worker_move_mask;
    let result_worker_mask = worker_move_mask ^ moving_worker_mask;

    if action.get_is_winning() {
        return vec![vec![
            PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
            PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        ]];
    }

    let build_position = action.build_position();
    return vec![vec![
        PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
        PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        PartialAction::Build(Square::from(build_position as usize)),
    ]];
}

pub fn athena_make_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = action.into();
    let worker_move_mask = action.move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.set_winner(board.current_player);
        return;
    }

    let build_position = action.build_position();
    let build_mask = BitBoard::as_mask(build_position);

    let build_height = board.get_height_for_worker(build_mask);
    board.height_map[build_height] |= build_mask;
    board.flip_worker_can_climb(!board.current_player, action.get_did_climb_change())
}

pub fn athena_unmake_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = unsafe { std::mem::transmute(action) };
    let worker_move_mask = action.move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.unset_winner();
        return;
    }

    let build_position = action.build_position();
    let build_mask = BitBoard::as_mask(build_position);

    let build_height = board.get_true_height(build_mask);
    board.height_map[build_height - 1] ^= build_mask;
    board.flip_worker_can_climb(!board.current_player, action.get_did_climb_change())
}

fn athena_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= board.exactly_level_2()
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];

    let did_not_improve_last_turn = board.get_worker_can_climb(!player);

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_own_workers =
                (current_workers ^ moving_worker_start_mask) & board.exactly_level_2();
            for other_pos in other_own_workers {
                neighbor_check_if_builds |=
                    NEIGHBOR_MAP[other_pos as usize] & board.exactly_level_2();
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    GodMove::new_athena_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                    )
                    .into(),
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
            let is_improving = worker_end_height > worker_starting_height;

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if !is_improving || (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let mut check_if_builds = neighbor_check_if_builds;
            let mut anti_check_builds = BitBoard::EMPTY;
            let mut is_already_check = false;

            if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                if worker_end_height == 2 {
                    check_if_builds |= worker_builds & board.exactly_level_2();
                    anti_check_builds =
                        NEIGHBOR_MAP[moving_worker_end_pos as usize] & board.exactly_level_3();
                    is_already_check = anti_check_builds != BitBoard::EMPTY;
                }
            }

            if F & GENERATE_THREATS_ONLY != 0 {
                if is_already_check {
                    let must_avoid_build = anti_check_builds & worker_builds;
                    if must_avoid_build.count_ones() == 1 {
                        worker_builds ^= must_avoid_build;
                    }
                } else {
                    worker_builds &= check_if_builds;
                }
            }

            for worker_build_pos in worker_builds {
                let new_action = GodMove::new_athena_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                    is_improving,
                    is_improving == did_not_improve_last_turn,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let score;
                    if is_already_check && (anti_check_builds & !worker_build_mask).is_not_empty()
                        || (worker_build_mask & check_if_builds).is_not_empty()
                    {
                        score = CHECK_SENTINEL_SCORE;
                    } else {
                        score = if is_improving {
                            IMPROVER_SENTINEL_SCORE
                        } else {
                            NON_IMPROVER_SENTINEL_SCORE
                        };
                    }
                    result.push(ScoredMove::new(new_action.into(), score));
                } else {
                    result.push(ScoredMove::new(new_action.into(), 0));
                }
            }
        }
    }

    result
}

pub fn athena_score_moves<const IMPROVERS_ONLY: bool>(
    board: &BoardState,
    move_list: &mut [ScoredMove],
) {
    let mut build_score_map: [MoveScore; 25] = [0; 25];
    for enemy_worker_pos in board.workers[1 - board.current_player as usize] {
        let enemy_worker_height = board.get_height_for_worker(BitBoard::as_mask(enemy_worker_pos));
        let ns = NEIGHBOR_MAP[enemy_worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height_for_worker(BitBoard::as_mask(n_pos));
            build_score_map[n_pos as usize] +=
                ENEMY_WORKER_BUILD_SCORES[enemy_worker_height as usize][n_height as usize];
        }
    }

    for worker_pos in board.workers[board.current_player as usize] {
        let worker_height = board.get_height_for_worker(BitBoard::as_mask(worker_pos));
        let ns = NEIGHBOR_MAP[worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height_for_worker(BitBoard::as_mask(n_pos));
            build_score_map[n_pos as usize] -=
                ENEMY_WORKER_BUILD_SCORES[worker_height as usize][n_height as usize] / 8;
        }
    }

    for scored_action in move_list {
        if IMPROVERS_ONLY && scored_action.score == NON_IMPROVER_SENTINEL_SCORE {
            continue;
        }

        let action: GodMove = scored_action.action.into();
        let mut score: MoveScore = 0;

        let from = action.move_from_position();
        let from_height = board.get_height_for_worker(BitBoard::as_mask(from));
        let to = action.move_to_position();
        let to_height = board.get_height_for_worker(BitBoard::as_mask(to));

        let build_at = action.build_position();
        let build_pre_height = board.get_height_for_worker(BitBoard::as_mask(build_at));

        score -= GRID_POSITION_SCORES[from as usize];
        score += GRID_POSITION_SCORES[to as usize];
        score -= WORKER_HEIGHT_SCORES[from_height as usize];
        score += WORKER_HEIGHT_SCORES[to_height as usize];

        score += build_score_map[build_at as usize];

        if scored_action.score == CHECK_SENTINEL_SCORE {
            score += CHECK_MOVE_BONUS;
        }

        if IMPROVERS_ONLY {
            score += IMPROVER_BUILD_HEIGHT_SCORES[to_height][build_pre_height];
        }

        scored_action.set_score(score);
    }
}

pub fn athena_blocker_board(action: GenericMove) -> BitBoard {
    let action: GodMove = action.into();
    BitBoard::as_mask(action.move_to_position())
}

pub fn athena_stringify(action: GenericMove) -> String {
    let action: GodMove = action.into();
    format!("{:?}", action)
}

pub const fn build_athena() -> GodPower {
    GodPower {
        god_name: GodName::Athena,
        _get_all_moves: athena_move_gen::<0>,
        _get_moves_for_search: athena_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        _get_wins: athena_move_gen::<{ MATE_ONLY }>,
        _get_win_blockers: athena_move_gen::<{ STOP_ON_MATE | INTERACT_WITH_KEY_SQUARES }>,
        _get_improver_moves_only: athena_move_gen::<
            { STOP_ON_MATE | GENERATE_THREATS_ONLY | INCLUDE_SCORE },
        >,
        get_actions_for_move: athena_move_to_actions,
        _score_improvers: athena_score_moves::<true>,
        _score_remaining: athena_score_moves::<false>,
        _get_blocker_board: athena_blocker_board,
        _make_move: athena_make_move,
        _unmake_move: athena_unmake_move,
        _stringify_move: athena_stringify,
    }
}

#[cfg(test)]
mod tests {
    use crate::random_utils::GameStateFuzzer;

    use super::*;

    #[test]
    fn test_athena_check_detection() {
        let athena = GodName::Athena.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = athena.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = athena.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                athena.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn =
                    athena.get_winning_moves(&board, current_player).len() > 0;

                if is_check_move != is_winning_next_turn {
                    println!(
                        "Failed check detection. Check guess: {:?}. Actual: {:?}",
                        is_check_move, is_winning_next_turn
                    );
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(is_check_move, is_winning_next_turn);
                }
            }
        }
    }

    #[test]
    fn test_athena_improver_checks_only() {
        let athena = GodName::Athena.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let current_player = state.board.current_player;

            if state.board.get_winner().is_some() {
                continue;
            }
            let current_win = athena.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let mut improver_moves = athena.get_improver_moves(&state.board, current_player);
            for action in &improver_moves {
                if action.score != CHECK_SENTINEL_SCORE {
                    let mut board = state.board.clone();
                    athena.make_move(&mut board, action.action);

                    println!("Move promised to be improver only but wasn't: {:?}", action,);
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(action.score, CHECK_SENTINEL_SCORE);
                }
            }

            let mut all_moves = athena.get_moves_for_search(&state.board, current_player);
            let check_count = all_moves
                .iter()
                .filter(|a| a.score == CHECK_SENTINEL_SCORE)
                .count();

            if improver_moves.len() != check_count {
                println!("Move count mismatch");
                state.board.print_to_console();
                println!("{:?}", state);

                improver_moves.sort_by_key(|a| -a.score);
                all_moves.sort_by_key(|a| -a.score);

                println!("IMPROVERS:");
                for a in &improver_moves {
                    println!("{:?}", a);
                }
                println!("ALL:");
                for a in &all_moves {
                    println!("{:?}", a);
                }

                assert_eq!(improver_moves.len(), check_count);
            }
        }
    }

    /*
    #[test]
    fn test_check_detection_move_into() {
        let athena = GodName::Athena.to_power();
        let state =
            FullGameState::try_from("11224 44444 00000 00000 00000/1/athena:A5,D5/athena:E1,E2")
                .unwrap();
        state.print_to_console();

        println!(
            "NON_IMPROVER_SENTINEL_SCORE: {}",
            NON_IMPROVER_SENTINEL_SCORE
        );
        println!("IMPROVER_SCORE: {}", IMPROVER_SENTINEL_SCORE);
        println!("CHECK_SCORE: {}", CHECK_SENTINEL_SCORE);

        let actions = athena.get_moves_for_search(&state.board, Player::One);
        for action in actions {
            println!("{:?}", action);
        }
    }
    */
}
