use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    build_god_power,
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
pub const APOLLO_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const APOLLO_MOVE_TO_POSITION_OFFSET: usize = APOLLO_MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_BUILD_POSITION_OFFSET: usize = APOLLO_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_DID_SWAP_OFFSET: usize = APOLLO_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_DID_SWAP_MASK: MoveData = 1 << APOLLO_DID_SWAP_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ApolloMove(pub MoveData);

impl Into<GenericMove> for ApolloMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ApolloMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ApolloMove {
    pub fn new_apollo_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        did_swap: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << APOLLO_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << APOLLO_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << APOLLO_BUILD_POSITION_OFFSET)
            | ((did_swap as MoveData) << APOLLO_DID_SWAP_OFFSET);

        Self(data)
    }

    pub fn new_apollo_winning_move(
        move_from_position: Square,
        move_to_position: Square,
        did_swap: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << APOLLO_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << APOLLO_MOVE_TO_POSITION_OFFSET)
            | ((did_swap as MoveData) << APOLLO_DID_SWAP_OFFSET)
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
        Square::from((self.0 >> APOLLO_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn did_swap(self) -> bool {
        self.0 & APOLLO_DID_SWAP_MASK != 0
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ApolloMove {
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
        } else if self.did_swap() {
            write!(f, "{}<>{}^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

type GodMove = ApolloMove;

pub fn apollo_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
    let action: GodMove = action.into();
    let current_player = board.current_player;
    let worker_move_mask = action.move_mask();
    let current_workers = board.workers[current_player as usize];

    let moving_worker_mask = current_workers & worker_move_mask;
    let result_worker_mask = worker_move_mask ^ moving_worker_mask;

    if action.get_is_winning() {
        return vec![vec![
            PartialAction::SelectWorker(moving_worker_mask.lsb()),
            PartialAction::MoveWorker(result_worker_mask.lsb()),
        ]];
    }

    let build_position = action.build_position();
    vec![vec![
        PartialAction::SelectWorker(moving_worker_mask.lsb()),
        PartialAction::MoveWorker(result_worker_mask.lsb()),
        PartialAction::Build(build_position),
    ]]
}

pub fn apollo_make_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = action.into();
    let worker_move_mask = action.move_mask();
    board.worker_xor(board.current_player, worker_move_mask);

    if action.did_swap() {
        board.worker_xor(!board.current_player, worker_move_mask);
    }

    if action.get_is_winning() {
        board.set_winner(board.current_player);
        return;
    }

    let build_position = action.build_position();
    board.build_up(build_position);
}

pub fn apollo_unmake_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = unsafe { std::mem::transmute(action) };
    let worker_move_mask = action.move_mask();
    board.worker_xor(board.current_player, worker_move_mask);

    if action.did_swap() {
        board.worker_xor(!board.current_player, worker_move_mask);
    }

    if action.get_is_winning() {
        board.unset_winner(board.current_player);
        return;
    }

    let build_position = action.build_position();
    board.unbuild(build_position);
}

fn apollo_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let base_current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let mut current_workers = base_current_workers;
    let opponent_workers = board.workers[1 - current_player_idx];

    let all_workers_mask = current_workers | opponent_workers;

    if F & MATE_ONLY != 0 {
        current_workers &= board.exactly_level_2()
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);
        let other_own_workers = current_workers ^ moving_worker_start_mask;
        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_lvl_2 = other_own_workers & board.exactly_level_2();
            for other_pos in other_lvl_2 {
                neighbor_check_if_builds |=
                    NEIGHBOR_MAP[other_pos as usize] & board.exactly_level_2();
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | base_current_workers);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let is_swap =
                    (BitBoard::as_mask(moving_worker_end_pos) & opponent_workers).is_not_empty();
                let winning_move = ScoredMove::new_winning_move(
                    GodMove::new_apollo_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        is_swap,
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
        let non_swapped_buildable_squares = !(non_selected_workers | board.height_map[3]);

        let swapped_buildable_squares = !(all_workers_mask | board.height_map[3]);

        let worker_builds_by_is_swap = [non_swapped_buildable_squares, swapped_buildable_squares];
        for moving_worker_end_pos in worker_moves.into_iter() {
            let is_swap =
                (BitBoard::as_mask(moving_worker_end_pos) & opponent_workers).is_not_empty();
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);

            let mut worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize]
                & worker_builds_by_is_swap[is_swap as usize];

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if !is_swap && (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let mut check_if_builds = neighbor_check_if_builds;
            let mut anti_check_builds = BitBoard::EMPTY;
            let mut is_already_check = false;

            if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                if worker_end_height == 2 {
                    check_if_builds |= worker_builds & board.exactly_level_2();
                    anti_check_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize]
                        & board.exactly_level_3()
                        & !other_own_workers;
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
                let new_action = GodMove::new_apollo_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                    is_swap,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let score;
                    if is_already_check && (anti_check_builds & !worker_build_mask).is_not_empty()
                        || (worker_build_mask & check_if_builds).is_not_empty()
                    {
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

    result
}

const APOLLO_SWAP_MOVE_BONUS: [MoveScore; 2] = [0, 9];

pub fn apollo_score_moves<const IMPROVERS_ONLY: bool>(
    board: &BoardState,
    move_list: &mut [ScoredMove],
) {
    let mut build_score_map: [MoveScore; 25] = [0; 25];
    for enemy_worker_pos in board.workers[1 - board.current_player as usize] {
        let enemy_worker_height = board.get_height(enemy_worker_pos);
        let ns = NEIGHBOR_MAP[enemy_worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height(n_pos);
            build_score_map[n_pos as usize] +=
                ENEMY_WORKER_BUILD_SCORES[enemy_worker_height as usize][n_height as usize];
        }
    }

    for worker_pos in board.workers[board.current_player as usize] {
        let worker_height = board.get_height(worker_pos);
        let ns = NEIGHBOR_MAP[worker_pos as usize];
        for n_pos in ns {
            let n_height = board.get_height(n_pos);
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

        score += APOLLO_SWAP_MOVE_BONUS[action.did_swap() as usize];

        let from = action.move_from_position();
        let from_height = board.get_height(from);
        let to = action.move_to_position();
        let to_height = board.get_height(to);

        let build_at = action.build_position();
        let build_pre_height = board.get_height(build_at);

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

pub fn apollo_blocker_board(action: GenericMove) -> BitBoard {
    let action: GodMove = action.into();
    BitBoard::as_mask(action.move_to_position())
}

pub fn apollo_stringify(action: GenericMove) -> String {
    let action: GodMove = action.into();
    format!("{:?}", action)
}

build_god_power!(
    build_apollo,
    god_name: GodName::Apollo,
    move_gen: apollo_move_gen,
    actions: apollo_move_to_actions,
    score_moves: apollo_score_moves,
    blocker_board: apollo_blocker_board,
    make_move: apollo_make_move,
    unmake_move: apollo_unmake_move,
    stringify: apollo_stringify,
);

#[cfg(test)]
mod tests {
    use crate::random_utils::GameStateFuzzer;

    use super::*;

    #[test]
    fn test_apollo_check_detection() {
        let apollo = GodName::Apollo.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = apollo.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = apollo.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                apollo.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn =
                    apollo.get_winning_moves(&board, current_player).len() > 0;

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
    fn test_apollo_improver_checks_only() {
        let apollo = GodName::Apollo.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let current_player = state.board.current_player;

            if state.board.get_winner().is_some() {
                continue;
            }
            let current_win = apollo.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let mut improver_moves = apollo.get_improver_moves(&state.board, current_player);
            for action in &improver_moves {
                if action.score != CHECK_SENTINEL_SCORE {
                    let mut board = state.board.clone();
                    apollo.make_move(&mut board, action.action);

                    println!("Move promised to be improver only but wasn't: {:?}", action,);
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(action.score, CHECK_SENTINEL_SCORE);
                }
            }

            let mut all_moves = apollo.get_moves_for_search(&state.board, current_player);
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
        let apollo = GodName::Apollo.to_power();
        let state =
            FullGameState::try_from("11224 44444 00000 00000 00000/1/apollo:A5,D5/apollo:E1,E2")
                .unwrap();
        state.print_to_console();

        println!(
            "NON_IMPROVER_SENTINEL_SCORE: {}",
            NON_IMPROVER_SENTINEL_SCORE
        );
        println!("IMPROVER_SCORE: {}", IMPROVER_SENTINEL_SCORE);
        println!("CHECK_SCORE: {}", CHECK_SENTINEL_SCORE);

        let actions = apollo.get_moves_for_search(&state.board, Player::One);
        for action in actions {
            println!("{:?}", action);
        }
    }
    */
}
