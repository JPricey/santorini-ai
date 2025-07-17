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
    utils::move_all_workers_one_include_original_workers,
};

use super::PartialAction;

pub const HERMES_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const HERMES_MOVE_TO_POSITION_OFFSET: usize = HERMES_MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_BUILD_POSITION_OFFSET: usize = HERMES_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_MOVE2_FROM_POSITION_OFFSET: usize = HERMES_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_MOVE2_TO_POSITION_OFFSET: usize =
    HERMES_MOVE2_FROM_POSITION_OFFSET + POSITION_WIDTH;

pub const HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_OFFSET: usize =
    HERMES_MOVE2_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_MASK: MoveData =
    1 << HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_OFFSET;

pub const HERMES_NOT_DOING_SPECIAL_MOVE_VALUE: MoveData = 25 << HERMES_MOVE2_FROM_POSITION_OFFSET;
pub const HERMES_NO_MOVE_MASK: BitBoard = BitBoard::as_mask_u8(0);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct HermesMove(pub MoveData);

impl Into<GenericMove> for HermesMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for HermesMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl HermesMove {
    pub fn new_hermes_no_move(build_position: Square) -> Self {
        let data: MoveData = ((build_position as MoveData) << HERMES_BUILD_POSITION_OFFSET)
            | HERMES_NOT_DOING_SPECIAL_MOVE_VALUE;

        Self(data)
    }

    pub fn new_hermes_single_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HERMES_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HERMES_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << HERMES_BUILD_POSITION_OFFSET)
            | HERMES_NOT_DOING_SPECIAL_MOVE_VALUE;

        Self(data)
    }

    pub fn new_hermes_double_move<const IS_OVERLAP: bool>(
        move_from_position: Square,
        move_to_position: Square,
        move_from2_position: Square,
        move_to2_position: Square,
        build_position: Square,
    ) -> Self {
        let mut data: MoveData = ((move_from_position as MoveData)
            << HERMES_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HERMES_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << HERMES_BUILD_POSITION_OFFSET)
            | ((move_from2_position as MoveData) << HERMES_MOVE2_FROM_POSITION_OFFSET)
            | ((move_to2_position as MoveData) << HERMES_MOVE2_TO_POSITION_OFFSET);

        if IS_OVERLAP {
            data |= HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_MASK;
        }

        Self(data)
    }

    pub fn new_hermes_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HERMES_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HERMES_MOVE_TO_POSITION_OFFSET)
            | HERMES_NOT_DOING_SPECIAL_MOVE_VALUE
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
        Square::from((self.0 >> HERMES_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_from_position2(&self) -> Option<Square> {
        let value = (self.0 >> HERMES_MOVE2_FROM_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    // WARNING: only returns usable values when move_from_position2 has returned a value
    pub fn move_to_position2(self) -> Square {
        Square::from((self.0 >> HERMES_MOVE2_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn are_double_moves_overlapping(self) -> bool {
        self.0 & HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_MASK != 0
    }

    pub fn move_mask(self) -> BitBoard {
        if let Some(move2) = self.move_from_position2() {
            BitBoard::as_mask(self.move_from_position())
                | BitBoard::as_mask(self.move_to_position())
                | BitBoard::as_mask(move2)
                | BitBoard::as_mask(self.move_to_position2())
        } else {
            let move_mask = BitBoard::as_mask(self.move_from_position())
                | BitBoard::as_mask(self.move_to_position());
            if move_mask == HERMES_NO_MOVE_MASK {
                BitBoard::EMPTY
            } else {
                move_mask
            }
        }
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for HermesMove {
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
        } else if let Some(move_from_2) = self.move_from_position2() {
            let move_to_2 = self.move_to_position2();
            write!(
                f,
                "{}>{} {}>{} ^{}",
                move_from, move_to, move_from_2, move_to_2, build
            )
        } else if move_to == move_from {
            write!(f, "^{}", build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

type GodMove = HermesMove;

pub fn hermes_move_to_actions(_board: &BoardState, action: GenericMove) -> Vec<FullAction> {
    let action: GodMove = action.into();

    if action.get_is_winning() {
        return vec![vec![
            PartialAction::SelectWorker(action.move_from_position()),
            PartialAction::MoveWorker(action.move_to_position()),
        ]];
    }
    let build_position = action.build_position();

    if action.move_from_position() == action.move_to_position() {
        return vec![vec![PartialAction::Build(build_position)]];
    }

    if let Some(from2) = action.move_from_position2() {
        let mut res = vec![
            vec![
                PartialAction::SelectWorker(action.move_from_position()),
                PartialAction::MoveWorker(action.move_to_position()),
                PartialAction::SelectWorker(from2),
                PartialAction::MoveWorker(action.move_to_position2()),
                PartialAction::Build(build_position),
            ],
            vec![
                PartialAction::SelectWorker(from2),
                PartialAction::MoveWorker(action.move_to_position2()),
                PartialAction::SelectWorker(action.move_from_position()),
                PartialAction::MoveWorker(action.move_to_position()),
                PartialAction::Build(build_position),
            ],
        ];

        if action.are_double_moves_overlapping() {
            res.push(vec![
                PartialAction::SelectWorker(from2),
                PartialAction::MoveWorker(action.move_to_position()),
                PartialAction::SelectWorker(action.move_from_position()),
                PartialAction::MoveWorker(action.move_to_position2()),
                PartialAction::Build(build_position),
            ]);
            res.push(vec![
                PartialAction::SelectWorker(from2),
                PartialAction::MoveWorker(action.move_to_position2()),
                PartialAction::SelectWorker(action.move_from_position()),
                PartialAction::MoveWorker(action.move_to_position()),
                PartialAction::Build(build_position),
            ]);
        }

        res
    } else {
        vec![vec![
            PartialAction::SelectWorker(action.move_from_position()),
            PartialAction::MoveWorker(action.move_to_position()),
            PartialAction::Build(build_position),
        ]]
    }
}

pub fn hermes_make_move(board: &mut BoardState, action: GenericMove) {
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
}

pub fn hermes_unmake_move(board: &mut BoardState, action: GenericMove) {
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
}

fn hermes_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let other_workers = board.workers[1 - current_player_idx] & BitBoard::MAIN_SECTION_MASK;

    if F & MATE_ONLY != 0 {
        current_workers &= board.exactly_level_2()
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);
    let all_workers_mask = board.workers[0] | board.workers[1];
    let can_climb = board.get_worker_can_climb(player);

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

        let mut worker_moves;
        if can_climb {
            if worker_starting_height == 3 {
                worker_moves = BitBoard::EMPTY
            } else {
                worker_moves = board.height_map[worker_starting_height]
                    & !board.height_map[worker_starting_height + 1]
            }
        } else if F & MATE_ONLY != 0 {
            continue;
        } else {
            worker_moves = BitBoard::EMPTY;
        };

        if worker_starting_height > 0 {
            worker_moves |= !board.height_map[worker_starting_height - 1]
        }

        worker_moves &= NEIGHBOR_MAP[moving_worker_start_pos as usize] & !all_workers_mask;

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    GodMove::new_hermes_winning_move(
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
                let new_action = GodMove::new_hermes_single_move(
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

    if F & MATE_ONLY != 0 {
        return result;
    }

    let mut worker_iter = current_workers;
    let f1 = worker_iter.next().unwrap();
    let m1 = BitBoard::as_mask(f1);
    let h1 = board.get_height_for_worker(m1);
    let h1_mask = board.exactly_level_n(h1) & !other_workers;

    let f2 = worker_iter.next().unwrap();
    let m2 = BitBoard::as_mask(f2);

    {
        // Handle neither worker moving
        let possible_builds = (NEIGHBOR_MAP[f1 as usize] | NEIGHBOR_MAP[f2 as usize])
            & !(all_workers_mask | board.height_map[3]);

        // TODO: check detection
        for build in possible_builds {
            let new_action = GodMove::new_hermes_no_move(build);
            result.push(ScoredMove::new(new_action.into(), 0));
        }
    }

    let mut c1 = m1;
    let mut component_size = c1.count_ones();
    loop {
        let old_size = component_size;
        c1 = move_all_workers_one_include_original_workers(c1) & h1_mask;
        component_size = c1.count_ones();
        if component_size == old_size {
            break;
        }
    }

    // Worker components overlap
    if (c1 & m2).is_not_empty() {
        c1 &= !all_workers_mask;
        // Move either worker
        for t in c1 {
            let t_mask = BitBoard::as_mask(t);
            // move f1
            {
                let possible_builds = (NEIGHBOR_MAP[t as usize] | NEIGHBOR_MAP[f2 as usize])
                    & !(other_workers | t_mask | m2 | board.height_map[3]);
                for build in possible_builds {
                    let new_action = GodMove::new_hermes_single_move(f1, t, build);
                    result.push(ScoredMove::new(new_action.into(), 0));
                }
            }

            // move f2
            {
                let possible_builds = (NEIGHBOR_MAP[t as usize] | NEIGHBOR_MAP[f1 as usize])
                    & !(other_workers | t_mask | m1 | board.height_map[3]);
                for build in possible_builds {
                    let new_action = GodMove::new_hermes_single_move(f2, t, build);
                    result.push(ScoredMove::new(new_action.into(), 0));
                }
            }
        }

        // Move both workers
        for t1 in c1 {
            c1 ^= BitBoard::as_mask(t1);
            let t1_mask = BitBoard::as_mask(t1);

            for t2 in c1 {
                let t2_mask = BitBoard::as_mask(t2);
                let both_mask = t1_mask | t2_mask;

                let possible_builds = (NEIGHBOR_MAP[t1 as usize] | NEIGHBOR_MAP[t2 as usize])
                    & !(other_workers | both_mask | board.height_map[3]);

                for build in possible_builds {
                    let new_action = GodMove::new_hermes_double_move::<true>(f1, t1, f2, t2, build);
                    result.push(ScoredMove::new(new_action.into(), 0));
                }
            }
        }
    } else {
        // worker components do not overlap
        let h2 = board.get_height_for_worker(m2);
        let h2_mask = board.exactly_level_n(h2) & !other_workers;
        let mut c2 = m2;
        let mut component_size = c2.count_ones();
        loop {
            let old_size = component_size;
            c2 = move_all_workers_one_include_original_workers(c2) & h2_mask;
            component_size = c1.count_ones();
            if component_size == old_size {
                break;
            }
        }
        c1 &= !all_workers_mask;
        c2 &= !all_workers_mask;

        // Move just 1
        for t in c1 {
            let t_mask = BitBoard::as_mask(t);
            let possible_builds = (NEIGHBOR_MAP[t as usize] | NEIGHBOR_MAP[f2 as usize])
                & !(other_workers | t_mask | m2 | board.height_map[3]);
            for build in possible_builds {
                let new_action = GodMove::new_hermes_single_move(f1, t, build);
                result.push(ScoredMove::new(new_action.into(), 0));
            }
        }
        // Move just 1
        for t in c2 {
            let t_mask = BitBoard::as_mask(t);
            let possible_builds = (NEIGHBOR_MAP[t as usize] | NEIGHBOR_MAP[f1 as usize])
                & !(other_workers | t_mask | m1 | board.height_map[3]);
            for build in possible_builds {
                let new_action = GodMove::new_hermes_single_move(f2, t, build);
                result.push(ScoredMove::new(new_action.into(), 0));
            }
        }

        // move both workers
        for t1 in c1 {
            let t1_mask = BitBoard::as_mask(t1);

            for t2 in c2 {
                let t2_mask = BitBoard::as_mask(t2);
                let both_mask = t1_mask | t2_mask;

                let possible_builds = (NEIGHBOR_MAP[t1 as usize] | NEIGHBOR_MAP[t2 as usize])
                    & !(other_workers | both_mask | board.height_map[3]);

                for build in possible_builds {
                    let new_action =
                        GodMove::new_hermes_double_move::<false>(f1, t1, f2, t2, build);
                    result.push(ScoredMove::new(new_action.into(), 0));
                }
            }
        }
    }

    // if result.len() >= 2236 {
    //     eprintln!("len: {}", result.len());
    // }

    result
}

pub fn hermes_score_moves<const IMPROVERS_ONLY: bool>(
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

pub fn hermes_blocker_board(action: GenericMove) -> BitBoard {
    let action: GodMove = action.into();
    BitBoard::as_mask(action.move_to_position())
}

pub fn hermes_stringify(action: GenericMove) -> String {
    let action: GodMove = action.into();
    format!("{:?}", action)
}

pub const fn build_hermes() -> GodPower {
    GodPower {
        god_name: GodName::Hermes,
        _get_all_moves: hermes_move_gen::<0>,
        _get_moves_for_search: hermes_move_gen::<{ STOP_ON_MATE | INCLUDE_SCORE }>,
        _get_wins: hermes_move_gen::<{ MATE_ONLY }>,
        _get_win_blockers: hermes_move_gen::<{ STOP_ON_MATE | INTERACT_WITH_KEY_SQUARES }>,
        _get_improver_moves_only: hermes_move_gen::<
            { STOP_ON_MATE | GENERATE_THREATS_ONLY | INCLUDE_SCORE },
        >,
        get_actions_for_move: hermes_move_to_actions,
        _score_improvers: hermes_score_moves::<true>,
        _score_remaining: hermes_score_moves::<false>,
        _get_blocker_board: hermes_blocker_board,
        _make_move: hermes_make_move,
        _unmake_move: hermes_unmake_move,
        _stringify_move: hermes_stringify,
    }
}

#[cfg(test)]
mod tests {
    use crate::random_utils::GameStateFuzzer;

    use super::*;

    #[test]
    fn test_hermes_check_detection() {
        let hermes = GodName::Hermes.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            if state.board.get_winner().is_some() {
                continue;
            }
            let current_player = state.board.current_player;
            let current_win = hermes.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let actions = hermes.get_moves_for_search(&state.board, current_player);
            for action in actions {
                let mut board = state.board.clone();
                hermes.make_move(&mut board, action.action);

                let is_check_move = action.score == CHECK_SENTINEL_SCORE;
                let is_winning_next_turn =
                    hermes.get_winning_moves(&board, current_player).len() > 0;

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
    fn test_hermes_improver_checks_only() {
        let hermes = GodName::Hermes.to_power();
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let current_player = state.board.current_player;

            if state.board.get_winner().is_some() {
                continue;
            }
            let current_win = hermes.get_winning_moves(&state.board, current_player);
            if current_win.len() != 0 {
                continue;
            }

            let mut improver_moves = hermes.get_improver_moves(&state.board, current_player);
            for action in &improver_moves {
                if action.score != CHECK_SENTINEL_SCORE {
                    let mut board = state.board.clone();
                    hermes.make_move(&mut board, action.action);

                    println!("Move promised to be improver only but wasn't: {:?}", action,);
                    println!("{:?}", state);
                    state.board.print_to_console();
                    println!("{:?}", action.action);
                    board.print_to_console();
                    assert_eq!(action.score, CHECK_SENTINEL_SCORE);
                }
            }

            let mut all_moves = hermes.get_moves_for_search(&state.board, current_player);
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
        let hermes = GodName::Hermes.to_power();
        let state =
            FullGameState::try_from("11224 44444 00000 00000 00000/1/hermes:A5,D5/hermes:E1,E2")
                .unwrap();
        state.print_to_console();

        println!(
            "NON_IMPROVER_SENTINEL_SCORE: {}",
            NON_IMPROVER_SENTINEL_SCORE
        );
        println!("IMPROVER_SCORE: {}", IMPROVER_SENTINEL_SCORE);
        println!("CHECK_SCORE: {}", CHECK_SENTINEL_SCORE);

        let actions = hermes.get_moves_for_search(&state.board, Player::One);
        for action in actions {
            println!("{:?}", action);
        }
    }
    */
}
