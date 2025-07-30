use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            CHECK_MOVE_BONUS, CHECK_SENTINEL_SCORE, ENEMY_WORKER_BUILD_SCORES, FULL_HEIGHT_MASK,
            FULL_HEIGHT_WIDTH, GENERATE_THREATS_ONLY, GRID_POSITION_SCORES, GenericMove,
            IMPROVER_BUILD_HEIGHT_SCORES, IMPROVER_SENTINEL_SCORE, INCLUDE_SCORE,
            INTERACT_WITH_KEY_SQUARES, LOWER_POSITION_MASK, MATE_ONLY, MOVE_IS_WINNING_MASK,
            MoveData, MoveGenFlags, MoveScore, NON_IMPROVER_SENTINEL_SCORE, NULL_MOVE_DATA,
            POSITION_WIDTH, STOP_ON_MATE, ScoredMove, WORKER_HEIGHT_SCORES,
        },
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

// from(5)|to(5)|build(5)|is_dome_build(1)
pub const ATLAS_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const ATLAS_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const ATLAS_BUILD_POSITION_OFFSET: usize = ATLAS_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const ATLAS_BUILD_OLD_HEIGHT_POSITION_OFFSET: usize =
    ATLAS_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const ATLAS_IS_DOME_BUILD_POSITION_OFFSET: usize =
    ATLAS_BUILD_OLD_HEIGHT_POSITION_OFFSET + FULL_HEIGHT_WIDTH;

pub const ATLAS_IS_DOME_BUILD_MASK: MoveData = 1 << ATLAS_IS_DOME_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
struct AtlasMove(pub MoveData);

impl Into<GenericMove> for AtlasMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AtlasMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AtlasMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        old_build_height: MoveData,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATLAS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATLAS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ATLAS_BUILD_POSITION_OFFSET)
            | ((old_build_height as MoveData) << ATLAS_BUILD_OLD_HEIGHT_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_dome_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        old_build_height: MoveData,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATLAS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATLAS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ATLAS_BUILD_POSITION_OFFSET)
            | ((old_build_height as MoveData) << ATLAS_BUILD_OLD_HEIGHT_POSITION_OFFSET)
            | ATLAS_IS_DOME_BUILD_MASK;

        Self(data)
    }

    pub fn new_atlas_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATLAS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATLAS_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> ATLAS_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn old_build_height(self) -> u8 {
        ((self.0 >> ATLAS_BUILD_OLD_HEIGHT_POSITION_OFFSET) as u8) & FULL_HEIGHT_MASK
    }

    pub fn is_dome_build(self) -> bool {
        self.0 & ATLAS_IS_DOME_BUILD_MASK != 0
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for AtlasMove {
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
        } else if self.is_dome_build() {
            write!(f, "{}>{}^{}X", move_from, move_to, build,)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build,)
        }
    }
}

type GodMove = AtlasMove;

fn atlas_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
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
    if action.is_dome_build() {
        return vec![vec![
            PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
            PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
            PartialAction::Build(Square::from(build_position as usize)),
            PartialAction::Build(Square::from(build_position as usize)),
        ]];
    } else {
        return vec![vec![
            PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
            PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
            PartialAction::Build(Square::from(build_position as usize)),
        ]];
    }
}

fn atlas_make_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = action.into();
    let worker_move_mask = action.move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.set_winner(board.current_player);
        return;
    }

    let build_position = action.build_position();
    let build_mask = BitBoard::as_mask(build_position);

    let old_build_height = action.old_build_height() as usize;
    if action.is_dome_build() {
        for i in old_build_height..4 {
            board.height_map[i] ^= build_mask;
        }
    } else {
        board.height_map[old_build_height] ^= build_mask;
    }
}

fn atlas_unmake_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = unsafe { std::mem::transmute(action) };
    let worker_move_mask = action.move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.unset_winner();
        return;
    }

    let build_position = action.build_position();
    let build_mask = BitBoard::as_mask(build_position);

    let old_build_height = action.old_build_height() as usize;
    if action.is_dome_build() {
        for i in old_build_height..4 {
            board.height_map[i] ^= build_mask;
        }
    } else {
        board.height_map[old_build_height] ^= build_mask;
    }
}

fn atlas_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let exactly_level_2 = board.exactly_level_2();
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= exactly_level_2
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];

    let can_dome_build_mask = !board.at_least_level_3();

    for moving_worker_start_pos in current_workers.into_iter() {
        // if moving_worker_start_pos != Square::C5 {
        //     continue;
        // }

        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_own_workers = (current_workers ^ moving_worker_start_mask) & exactly_level_2;
            for other_pos in other_own_workers {
                let neighbors = NEIGHBOR_MAP[other_pos as usize];
                neighbor_check_if_builds |= neighbors & exactly_level_2;
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
                    GodMove::new_atlas_winning_move(moving_worker_start_pos, moving_worker_end_pos)
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
            // if moving_worker_end_pos != Square::D5 {
            //     continue;
            // }

            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);

            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let mut check_if_builds = neighbor_check_if_builds;
            let mut anti_check_builds = BitBoard::EMPTY;
            let mut is_already_check = false;

            if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                if worker_end_height == 2 {
                    check_if_builds |= worker_builds & exactly_level_2;
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
                // if worker_build_pos != Square::D4 {
                //     continue;
                // }

                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let worker_build_height = board.get_height_for_worker(worker_build_mask);

                let new_action = GodMove::new_basic_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                    worker_build_height as MoveData,
                );
                let is_move_based_check =
                    is_already_check && (anti_check_builds & !worker_build_mask).is_not_empty();
                if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                    let score;
                    if is_move_based_check || (worker_build_mask & check_if_builds).is_not_empty() {
                        score = CHECK_SENTINEL_SCORE;
                    } else {
                        let is_improving = worker_end_height > worker_starting_height;
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

                if F & GENERATE_THREATS_ONLY != 0 && !is_move_based_check {
                    continue;
                }

                if (worker_build_mask & can_dome_build_mask).is_not_empty() {
                    let new_action = GodMove::new_dome_build_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        worker_build_pos,
                        worker_build_height as MoveData,
                    );
                    if F & INCLUDE_SCORE != 0 {
                        let score;
                        if is_move_based_check {
                            score = CHECK_SENTINEL_SCORE;
                        } else {
                            let is_improving = worker_end_height > worker_starting_height;
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
    }

    result
}

fn atlas_score_moves<const IMPROVERS_ONLY: bool>(board: &BoardState, move_list: &mut [ScoredMove]) {
    let mut build_score_map: [MoveScore; 25] = [0; 25];
    let mut dome_score_map: [MoveScore; 25] = [17; 25];

    for enemy_worker_pos in board.workers[1 - board.current_player as usize] {
        let enemy_worker_height = board.get_height_for_worker(BitBoard::as_mask(enemy_worker_pos));
        let ns = NEIGHBOR_MAP[enemy_worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height_for_worker(BitBoard::as_mask(n_pos));
            build_score_map[n_pos as usize] +=
                ENEMY_WORKER_BUILD_SCORES[enemy_worker_height as usize][n_height as usize];
            dome_score_map[n_pos as usize] += 32;
        }
    }

    for worker_pos in board.workers[board.current_player as usize] {
        let worker_height = board.get_height_for_worker(BitBoard::as_mask(worker_pos));
        let ns = NEIGHBOR_MAP[worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height_for_worker(BitBoard::as_mask(n_pos));
            build_score_map[n_pos as usize] -=
                ENEMY_WORKER_BUILD_SCORES[worker_height as usize][n_height as usize] / 8;
            dome_score_map[n_pos as usize] -= 10;
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

        score -= GRID_POSITION_SCORES[from as usize];
        score += GRID_POSITION_SCORES[to as usize];
        score -= WORKER_HEIGHT_SCORES[from_height as usize];
        score += WORKER_HEIGHT_SCORES[to_height as usize];

        if action.is_dome_build() {
            score += dome_score_map[build_at as usize];
        } else {
            score += build_score_map[build_at as usize];

            if IMPROVERS_ONLY {
                let build_pre_height = board.get_height_for_worker(BitBoard::as_mask(build_at));
                score += IMPROVER_BUILD_HEIGHT_SCORES[to_height][build_pre_height];
            }
        }

        if scored_action.score == CHECK_SENTINEL_SCORE {
            score += CHECK_MOVE_BONUS;
        }

        scored_action.set_score(score);
    }
}

fn atlas_blocker_board(action: GenericMove) -> BitBoard {
    let action: GodMove = action.into();
    BitBoard::as_mask(action.move_to_position())
}

fn atlas_stringify(action: GenericMove) -> String {
    let action: GodMove = action.into();
    format!("{:?}", action)
}

pub const fn build_atlas() -> GodPower {
    GodPower {
        god_name: GodName::Atlas,
        _get_all_moves: atlas_move_gen::<0>,
        _get_moves_for_search: atlas_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        _get_wins: atlas_move_gen::<{ MATE_ONLY }>,
        _get_win_blockers: atlas_move_gen::<{ STOP_ON_MATE | INTERACT_WITH_KEY_SQUARES }>,
        _get_improver_moves_only: atlas_move_gen::<
            { STOP_ON_MATE | GENERATE_THREATS_ONLY | INCLUDE_SCORE },
        >,
        get_actions_for_move: atlas_move_to_actions,
        _score_improvers: atlas_score_moves::<true>,
        _score_remaining: atlas_score_moves::<false>,
        _get_blocker_board: atlas_blocker_board,
        _make_move: atlas_make_move,
        _unmake_move: atlas_unmake_move,
        _stringify_move: atlas_stringify,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        board::{self, FullGameState},
        random_utils::GameStateFuzzer,
    };

    use super::*;

    #[test]
    fn test_atlas_check_detection() {
        let atlas = GodName::Atlas.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = atlas.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = atlas.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                atlas.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn =
                    atlas.get_winning_moves(&board, current_player).len() > 0;

                if is_check_move != is_winning_next_turn {
                    println!(
                        "Failed check detection. Check guess: {:?}. Actual: {:?}",
                        is_check_move, is_winning_next_turn
                    );
                    println!("{:?}", state);
                    state.board.print_to_console();
                    let acc: GodMove = action.action.into();
                    println!("{:?} {:b}", acc, acc.0);
                    board.print_to_console();
                    assert_eq!(is_check_move, is_winning_next_turn);
                }
            }
        }
    }

    #[test]
    fn test_atlas_improver_checks_only() {
        let atlas = GodName::Atlas.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let current_player = state.board.current_player;

            if state.board.get_winner().is_some() {
                continue;
            }
            let current_win = atlas.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let mut improver_moves = atlas.get_improver_moves(&state.board, current_player);
            for action in &improver_moves {
                if action.score != CHECK_SENTINEL_SCORE {
                    let mut board = state.board.clone();
                    atlas.make_move(&mut board, action.action);

                    println!("Move promised to be improver only but wasn't: {:?}", action);
                    println!("{:?}", state);
                    state.board.print_to_console();
                    let acc: GodMove = action.action.into();
                    println!("{:?}", acc);
                    board.print_to_console();
                    assert_eq!(action.score, CHECK_SENTINEL_SCORE);
                }
            }

            let mut all_moves = atlas.get_moves_for_search(&state.board, current_player);
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

    #[test]
    fn debug_atlas_move() {
        let atlas = GodName::Atlas.to_power();
        let state =
            FullGameState::try_from("0012000020000000000000001/1/mortal:C5,D2/mortal:A3,E5")
                .unwrap();
        state.print_to_console();

        println!(
            "NON_IMPROVER_SENTINEL_SCORE: {}",
            NON_IMPROVER_SENTINEL_SCORE
        );
        println!("IMPROVER_SCORE: {}", IMPROVER_SENTINEL_SCORE);
        println!("CHECK_SCORE: {}", CHECK_SENTINEL_SCORE);

        let actions = atlas.get_moves_for_search(&state.board, Player::One);
        for action in actions {
            let acc: GodMove = action.action.into();
            println!("{:?} : {}", acc, action.score);
        }
    }

    #[test]
    fn test_atlas_make_unmake() {
        let atlas = GodName::Atlas.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let orig_board = state.board.clone();
            let child_actions =
                (atlas._get_all_moves)(&orig_board, orig_board.current_player, BitBoard::EMPTY);

            for action in child_actions {
                let mut board = orig_board.clone();
                let action = action.action;
                atlas.make_move(&mut board, action);
                board.validate_heights();
                atlas.unmake_move(&mut board, action);
                board.validate_heights();
                assert_eq!(board, orig_board);
            }
        }
    }
}
