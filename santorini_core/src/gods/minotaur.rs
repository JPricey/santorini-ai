use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, NEIGHBOR_MAP},
    build_god_power,
    gods::{
        generic::{
            GenericMove, GodMove, MoveData, MoveGenFlags, ScoredMove, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, LOWER_POSITION_MASK, MATE_ONLY, MOVE_IS_WINNING_MASK, NULL_MOVE_DATA, POSITION_WIDTH, STOP_ON_MATE
        }, FullAction, GodName, GodPower
    },
    player::Player,
    square::Square,
    transmute_enum,
};

use super::PartialAction;

const MINOTAUR_PUSH_TO_MAPPING: [[Option<Square>; 25]; 25] = {
    let mut result = [[None; 25]; 25];

    let mut from: i32 = 0;
    loop {
        if from >= 25 {
            break;
        }

        let mut to: i32 = 0;
        loop {
            if to >= 25 {
                break;
            }
            let to_mask = BitBoard::as_mask(transmute_enum!(to as u8));
            if (NEIGHBOR_MAP[from as usize].0 & to_mask.0) != 0 {
                let delta = to - from;
                let dest = to + delta;
                if dest >= 0 && dest < 25 {
                    if NEIGHBOR_MAP[to as usize].0 & 1 << dest != 0 {
                        result[from as usize][to as usize] = Some(transmute_enum!(dest as u8));
                    }
                }
            }
            to += 1;
        }
        from += 1;
    }

    result
};

// from(5)|to(5)|build(5)|win(1)
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

    pub fn new_minotaur_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
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

    fn unmake_move(self, board: &mut BoardState) {
        let move_from = BitBoard::as_mask(self.move_from_position());
        let move_to = BitBoard::as_mask(self.move_to_position());
        board.worker_xor(board.current_player, move_to | move_from);

        if self.get_is_winning() {
            board.unset_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.unbuild(build_position);

        if let Some(push_to) = self.push_to_position() {
            let push_mask = BitBoard::as_mask(push_to);
            board.worker_xor(!board.current_player, move_to | push_mask);
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
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
    let board = &state.board;
    let current_player_idx = player as usize;
    let base_current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let mut current_workers = base_current_workers;
    if F & MATE_ONLY != 0 {
        current_workers &= board.exactly_level_2()
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };
    let opponent_workers = board.workers[1 - current_player_idx];

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    let all_workers_mask = board.workers[0] | board.workers[1];
    let blocked_squares = all_workers_mask | board.at_least_level_4();

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);
        let base_other_own_workers = current_workers ^ moving_worker_start_mask;

        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_own_workers = base_other_own_workers & board.exactly_level_2();
            for other_pos in other_own_workers {
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
                let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
                if (moving_worker_end_mask & opponent_workers).is_not_empty() {
                    if let Some(push_to) = MINOTAUR_PUSH_TO_MAPPING
                        [moving_worker_start_pos as usize][moving_worker_end_pos as usize]
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
                            if F & STOP_ON_MATE != 0 {
                                return result;
                            }
                        }
                    }
                } else {
                    let winning_move = ScoredMove::new_winning_move(
                        MinotaurMove::new_minotaur_winning_move(
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
        }

        if F & MATE_ONLY != 0 {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;
            let mut push_to_spot: Option<Square> = None;
            let mut push_to_mask = BitBoard::EMPTY;

            if (moving_worker_end_mask & opponent_workers).is_not_empty() {
                if let Some(push_to) = MINOTAUR_PUSH_TO_MAPPING[moving_worker_start_pos as usize]
                    [moving_worker_end_pos as usize]
                {
                    let tmp_push_to_mask = BitBoard::as_mask(push_to);
                    if (tmp_push_to_mask & blocked_squares).is_empty() {
                        push_to_spot = Some(push_to);
                        push_to_mask = tmp_push_to_mask;
                        worker_builds ^= tmp_push_to_mask;
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let mut check_if_builds = neighbor_check_if_builds;
            check_if_builds &= !push_to_mask;
            let mut anti_check_builds = BitBoard::EMPTY;
            let mut is_already_check = false;
            let mut is_already_check_by_push = false;

            if F & (INCLUDE_SCORE) != 0 {
                if worker_end_height == 2 {
                    check_if_builds |= worker_builds & board.exactly_level_2();
                    anti_check_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize]
                        & board.exactly_level_3()
                        & !base_other_own_workers;
                    let anti_check_pushes = anti_check_builds & (push_to_mask | opponent_workers);
                    anti_check_builds ^= anti_check_pushes;

                    for anti_check_push_pos in anti_check_pushes {
                        if let Some(push_to) = MINOTAUR_PUSH_TO_MAPPING
                            [moving_worker_end_pos as usize]
                            [anti_check_push_pos as usize]
                        {
                            let push_to_mask = BitBoard::as_mask(push_to);
                            if (push_to_mask & blocked_squares).is_empty() {
                                is_already_check_by_push = true;
                            }
                        }
                    }

                    is_already_check = anti_check_builds != BitBoard::EMPTY;
                }
            }

            for worker_build_pos in worker_builds {
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
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    if is_already_check_by_push
                        || is_already_check
                            && (anti_check_builds & !worker_build_mask).is_not_empty()
                        || (worker_build_mask & check_if_builds).is_not_empty()
                    {
                        result.push(ScoredMove::new_checking_move(new_action.into()));
                    } else {
                        let is_improving = worker_end_height > worker_starting_height;
                        if is_improving {
                            result.push(ScoredMove::new_improving_move(new_action.into()));
                        } else {
                            result.push(ScoredMove::new_non_improver(new_action.into()));
                        };
                    }
                } else {
                    result.push(ScoredMove::new_unscored_move(new_action.into()));
                }
            }
        }
    }

    result
}

build_god_power!(
    build_minotaur,
    god_name: GodName::Minotaur,
    move_type: MinotaurMove,
    move_gen: minotaur_move_gen,
    hash1: 16532879311019593353,
    hash2: 196173323035994051,
);
