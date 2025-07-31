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

pub const PROMETHEUS_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const PROMETHEUS_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const PROMETHEUS_BUILD_POSITION_OFFSET: usize =
    PROMETHEUS_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const PROMETHEUS_PRE_BUILD_POSITION_OFFSET: usize =
    PROMETHEUS_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_OFFSET: usize =
    PROMETHEUS_PRE_BUILD_POSITION_OFFSET + POSITION_WIDTH;

pub const PROMETHEUS_NO_PRE_BUILD_VALUE: MoveData = 25 << PROMETHEUS_PRE_BUILD_POSITION_OFFSET;
pub const PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_VALUE: MoveData =
    1 << PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PrometheusMove(pub MoveData);

impl Into<GenericMove> for PrometheusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for PrometheusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl PrometheusMove {
    pub fn new_prometheus_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << PROMETHEUS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << PROMETHEUS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << PROMETHEUS_BUILD_POSITION_OFFSET)
            | PROMETHEUS_NO_PRE_BUILD_VALUE;

        Self(data)
    }

    pub fn new_prometheus_two_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        pre_build_position: Square,
        is_interchangeable: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << PROMETHEUS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << PROMETHEUS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << PROMETHEUS_BUILD_POSITION_OFFSET)
            | ((pre_build_position as MoveData) << PROMETHEUS_PRE_BUILD_POSITION_OFFSET)
            | ((is_interchangeable as MoveData) << PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_OFFSET);

        Self(data)
    }

    pub fn new_prometheus_winning_move(
        move_from_position: Square,
        move_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << PROMETHEUS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << PROMETHEUS_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> PROMETHEUS_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn pre_build_position(self) -> Option<Square> {
        let value = (self.0 >> PROMETHEUS_PRE_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn are_builds_interchangeable(self) -> bool {
        self.0 & PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_VALUE != 0
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for PrometheusMove {
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
        } else if let Some(pre_build) = self.pre_build_position() {
            write!(f, "^{} {}>{}^{}", pre_build, move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

type GodMove = PrometheusMove;

pub fn prometheus_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
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

    if let Some(pre_build_position) = action.pre_build_position() {
        let mut res = vec![vec![
            PartialAction::Build(pre_build_position),
            PartialAction::SelectWorker(moving_worker_mask.lsb()),
            PartialAction::MoveWorker(result_worker_mask.lsb()),
            PartialAction::Build(build_position),
        ]];
        if action.are_builds_interchangeable() {
            res.push(vec![
                PartialAction::Build(build_position),
                PartialAction::SelectWorker(moving_worker_mask.lsb()),
                PartialAction::MoveWorker(result_worker_mask.lsb()),
                PartialAction::Build(pre_build_position),
            ]);
        }

        res
    } else {
        vec![vec![
            PartialAction::SelectWorker(moving_worker_mask.lsb()),
            PartialAction::MoveWorker(result_worker_mask.lsb()),
            PartialAction::Build(build_position),
        ]]
    }
}

pub fn prometheus_make_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = action.into();
    let worker_move_mask = action.move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.set_winner(board.current_player);
        return;
    }

    {
        let build_position = action.build_position();
        let build_mask = BitBoard::as_mask(build_position);
        let build_height = board.get_height_for_worker(build_mask);
        board.height_map[build_height] |= build_mask;
    }

    if let Some(build_position) = action.pre_build_position() {
        let build_mask = BitBoard::as_mask(build_position);
        let build_height = board.get_height_for_worker(build_mask);
        board.height_map[build_height] |= build_mask;
    }
}

pub fn prometheus_unmake_move(board: &mut BoardState, action: GenericMove) {
    let action: GodMove = unsafe { std::mem::transmute(action) };
    let worker_move_mask = action.move_mask();
    board.workers[board.current_player as usize] ^= worker_move_mask;

    if action.get_is_winning() {
        board.unset_winner();
        return;
    }

    {
        let build_position = action.build_position();
        let build_mask = BitBoard::as_mask(build_position);
        let build_height = board.get_true_height(build_mask);
        board.height_map[build_height - 1] ^= build_mask;
    }

    if let Some(build_position) = action.pre_build_position() {
        let build_mask = BitBoard::as_mask(build_position);
        let build_height = board.get_true_height(build_mask);
        board.height_map[build_height - 1] ^= build_mask;
    }
}

fn prometheus_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;

    let board_exactly_1 = board.exactly_level_1();
    let board_exactly_2 = board.exactly_level_2();
    let board_exactly_3 = board.exactly_level_3();

    if F & MATE_ONLY != 0 {
        current_workers &= board_exactly_2;
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let worker_starting_neighbors = NEIGHBOR_MAP[moving_worker_start_pos as usize];

        let mut worker_moves = worker_starting_neighbors
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    GodMove::new_prometheus_winning_move(
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

        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        let mut neighbor_check_if_double_builds = BitBoard::EMPTY;

        if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
            let other_own_workers = (current_workers ^ moving_worker_start_mask) & board_exactly_2;
            for other_pos in other_own_workers {
                let other_neighbors = NEIGHBOR_MAP[other_pos as usize];
                neighbor_check_if_builds |= other_neighbors & board_exactly_2;
                neighbor_check_if_double_builds |= other_neighbors & board_exactly_1;
            }
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let all_buildable_squares = !(non_selected_workers | board.height_map[3]);

        let pre_build_locations = worker_starting_neighbors & all_buildable_squares;

        let pre_build_worker_moves = worker_moves & board.exactly_level_n(worker_starting_height);
        let exactly_one_less = if worker_starting_height == 0 {
            BitBoard::EMPTY
        } else {
            board.exactly_level_n(worker_starting_height - 1)
        };

        for pre_build_pos in pre_build_locations {
            let pre_build_mask = BitBoard::as_mask(pre_build_pos);
            let not_pre_build_mask = !pre_build_mask;

            let pre_build_worker_moves =
                pre_build_worker_moves & !pre_build_mask | pre_build_mask & exactly_one_less;

            for moving_worker_end_pos in pre_build_worker_moves.into_iter() {
                let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
                let worker_end_height = board.get_height_for_worker(moving_worker_end_mask)
                    + ((moving_worker_end_pos == pre_build_pos) as usize);

                let mut worker_builds =
                    NEIGHBOR_MAP[moving_worker_end_pos as usize] & all_buildable_squares;
                let both_buildable = worker_builds & pre_build_locations;
                worker_builds &= !(pre_build_mask & board_exactly_3);

                if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                    if ((moving_worker_end_mask | pre_build_mask) & key_squares).is_empty() {
                        worker_builds = worker_builds & key_squares;
                    }
                }

                let mut neighbor_check_if_builds = neighbor_check_if_builds;
                let mut check_if_builds;
                let mut anti_check_builds = BitBoard::EMPTY;
                let mut is_already_check = false;

                if F & (INCLUDE_SCORE | GENERATE_THREATS_ONLY) != 0 {
                    // if the pre-build generated a neighbor check
                    anti_check_builds = neighbor_check_if_builds & pre_build_mask;
                    is_already_check = anti_check_builds.is_not_empty();
                    neighbor_check_if_builds &= not_pre_build_mask;
                    neighbor_check_if_builds |= neighbor_check_if_double_builds & pre_build_mask;

                    check_if_builds = neighbor_check_if_builds;

                    if worker_end_height == 2 {
                        check_if_builds |= worker_builds
                            & ((not_pre_build_mask & board_exactly_2)
                                | (pre_build_mask & board_exactly_1));

                        anti_check_builds |= NEIGHBOR_MAP[moving_worker_end_pos as usize]
                            & all_buildable_squares
                            & (not_pre_build_mask & board_exactly_3
                                | pre_build_mask & board_exactly_2);

                        is_already_check = is_already_check || anti_check_builds != BitBoard::EMPTY;
                    }
                } else {
                    check_if_builds = BitBoard::EMPTY;
                }

                if F & GENERATE_THREATS_ONLY != 0 {
                    if is_already_check {
                        if anti_check_builds.count_ones() == 1 {
                            worker_builds &= !anti_check_builds;
                        }
                    } else {
                        worker_builds &= check_if_builds;
                    }
                }

                for worker_build_pos in worker_builds {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);

                    let is_either_order = pre_build_mask != worker_build_mask
                        && (both_buildable | pre_build_mask | worker_build_mask) == both_buildable;

                    // avoid duplicates
                    if is_either_order && pre_build_pos > worker_build_pos {
                        continue;
                    }

                    let new_action = GodMove::new_prometheus_two_build_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        worker_build_pos,
                        pre_build_pos,
                        is_either_order,
                    );

                    if F & INCLUDE_SCORE != 0 {
                        let score;
                        if is_already_check
                            && (anti_check_builds & !worker_build_mask).is_not_empty()
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

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & all_buildable_squares;

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
                    check_if_builds |= worker_builds & board_exactly_2;
                    anti_check_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize]
                        & board_exactly_3
                        & all_buildable_squares;
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
                let new_action = GodMove::new_prometheus_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
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

pub fn prometheus_score_moves<const IMPROVERS_ONLY: bool>(
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
        let mut score: i32 = 0;

        let from = action.move_from_position();
        let from_height = board.get_height_for_worker(BitBoard::as_mask(from));
        let to = action.move_to_position();
        let to_height = board.get_height_for_worker(BitBoard::as_mask(to));

        score -= GRID_POSITION_SCORES[from as usize] as i32;
        score += GRID_POSITION_SCORES[to as usize] as i32;
        score -= WORKER_HEIGHT_SCORES[from_height as usize] as i32;
        score += WORKER_HEIGHT_SCORES[to_height as usize] as i32;

        let build_at = action.build_position();
        let build_pre_height = board.get_height_for_worker(BitBoard::as_mask(build_at));
        score += build_score_map[build_at as usize] as i32;
        if IMPROVERS_ONLY {
            score += IMPROVER_BUILD_HEIGHT_SCORES[to_height][build_pre_height] as i32;
        }

        if let Some(pre_build_at) = action.pre_build_position() {
            let build_pre_height = board.get_height_for_worker(BitBoard::as_mask(pre_build_at));
            score += build_score_map[pre_build_at as usize] as i32;
            if IMPROVERS_ONLY {
                if pre_build_at == build_at {
                    score -= IMPROVER_BUILD_HEIGHT_SCORES[to_height][build_pre_height] as i32;
                    score += IMPROVER_BUILD_HEIGHT_SCORES[to_height][build_pre_height + 1] as i32;
                } else {
                    score += IMPROVER_BUILD_HEIGHT_SCORES[to_height][build_pre_height] as i32;
                }
            }
        }

        if scored_action.score == CHECK_SENTINEL_SCORE {
            score += CHECK_MOVE_BONUS as i32;
        }

        score = score.clamp((CHECK_SENTINEL_SCORE + 1) as i32, MoveScore::MAX as i32);
        scored_action.set_score(score as MoveScore);
    }
}

pub fn prometheus_blocker_board(action: GenericMove) -> BitBoard {
    let action: GodMove = action.into();
    BitBoard::as_mask(action.move_to_position())
}

pub fn prometheus_stringify(action: GenericMove) -> String {
    let action: GodMove = action.into();
    format!("{:?}", action)
}

build_god_power!(
    build_prometheus,
    god_name: GodName::Prometheus,
    move_gen: prometheus_move_gen,
    actions: prometheus_move_to_actions,
    score_moves: prometheus_score_moves,
    blocker_board: prometheus_blocker_board,
    make_move: prometheus_make_move,
    unmake_move: prometheus_unmake_move,
    stringify: prometheus_stringify,
);

#[cfg(test)]
mod tests {
    use crate::{
        board::{self, FullGameState},
        random_utils::GameStateFuzzer,
    };

    use super::*;

    #[test]
    fn test_prometheus_check_detection() {
        let prometheus = GodName::Prometheus.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = prometheus.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = prometheus.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                prometheus.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn =
                    prometheus.get_winning_moves(&board, current_player).len() > 0;

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
    fn test_prometheus_improver_checks_only() {
        let prometheus = GodName::Prometheus.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let current_player = state.board.current_player;

            if state.board.get_winner().is_some() {
                continue;
            }
            let current_win = prometheus.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let mut improver_moves = prometheus.get_improver_moves(&state.board, current_player);
            for action in &improver_moves {
                if action.score != CHECK_SENTINEL_SCORE {
                    let mut board = state.board.clone();
                    prometheus.make_move(&mut board, action.action);

                    println!("Move promised to be improver only but wasn't: {:?}", action);
                    println!("{:?}", state);
                    state.board.print_to_console();
                    let acc: GodMove = action.action.into();
                    println!("{:?}", acc);
                    board.print_to_console();
                    assert_eq!(action.score, CHECK_SENTINEL_SCORE);
                }
            }

            let mut all_moves = prometheus.get_moves_for_search(&state.board, current_player);
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
    fn debug_prometheus_move() {
        let prometheus = GodName::Prometheus.to_power();
        // let state = FullGameState::try_from("00000 22444 000000000000000/1/prometheus:A5/prometheus:E1").unwrap();
        let state =
            FullGameState::try_from("0000224310012100302000100/1/prometheus:A3/hephaestus:A5,D5")
                .unwrap();
        state.print_to_console();

        println!(
            "NON_IMPROVER_SENTINEL_SCORE: {}",
            NON_IMPROVER_SENTINEL_SCORE
        );
        println!("IMPROVER_SCORE: {}", IMPROVER_SENTINEL_SCORE);
        println!("CHECK_SCORE: {}", CHECK_SENTINEL_SCORE);

        let actions = prometheus.get_moves_for_search(&state.board, Player::One);
        for action in actions {
            let acc: GodMove = action.action.into();
            println!("{:?} : {}", acc, action.score);
        }
    }

    #[test]
    fn test_prometheus_make_unmake() {
        let prometheus = GodName::Prometheus.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let orig_board = state.board.clone();
            let child_actions = (prometheus._get_all_moves)(
                &orig_board,
                orig_board.current_player,
                BitBoard::EMPTY,
            );

            for action in child_actions {
                let mut board = orig_board.clone();
                let action = action.action;
                prometheus.make_move(&mut board, action);
                board.validate_heights();
                prometheus.unmake_move(&mut board, action);
                board.validate_heights();
                assert_eq!(board, orig_board);
            }
        }
    }
}
