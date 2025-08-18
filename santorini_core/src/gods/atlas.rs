use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    build_god_power,
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            FULL_HEIGHT_MASK, FULL_HEIGHT_WIDTH, GenericMove, GodMove, INCLUDE_SCORE,
            INTERACT_WITH_KEY_SQUARES, LOWER_POSITION_MASK, MATE_ONLY, MOVE_IS_WINNING_MASK,
            MoveData, MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, STOP_ON_MATE, ScoredMove,
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

impl GodMove for AtlasMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position()),
        ];

        if self.get_is_winning() {
            return vec![res];
        }

        let build_position = self.build_position();
        res.push(PartialAction::Build(build_position));

        if self.is_dome_build() {
            res.push(PartialAction::Dome(build_position));
        }

        return vec![res];
    }

    fn make_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        if self.is_dome_build() {
            board.dome_up(build_position);
        } else {
            board.build_up(build_position);
        }
    }

    fn unmake_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.unset_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        if self.is_dome_build() {
            board.undome(build_position, self.old_build_height() as usize);
        } else {
            board.unbuild(build_position);
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
        res = res * 2 + self.is_dome_build() as usize;

        res
    }
}

fn atlas_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let exactly_level_2 = board.exactly_level_2();
    let exactly_level_3 = board.exactly_level_3();
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    if F & MATE_ONLY != 0 {
        current_workers &= exactly_level_2
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);
    let all_workers_mask = board.workers[0] | board.workers[1];
    let can_dome_build_mask = !board.at_least_level_3();

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let mut neighbor_neighbor = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_checkable_workers =
                (current_workers ^ moving_worker_start_mask) & exactly_level_2;
            for other_pos in other_checkable_workers {
                neighbor_neighbor |= NEIGHBOR_MAP[other_pos as usize];
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
                    AtlasMove::new_atlas_winning_move(
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
            let worker_end_height = board.get_height(moving_worker_end_pos);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;
            let worker_plausible_next_moves = worker_builds;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let reach_board = neighbor_neighbor
                | (worker_plausible_next_moves
                    & BitBoard::CONDITIONAL_MASK[(worker_end_height == 2) as usize]);
            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let worker_build_height = board.get_height(worker_build_pos);

                {
                    let new_action = AtlasMove::new_basic_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        worker_build_pos,
                        worker_build_height as MoveData,
                    );
                    if F & INCLUDE_SCORE != 0 {
                        let final_level_3 = (exactly_level_2 & worker_build_mask)
                            | (exactly_level_3 & !worker_build_mask);
                        let check_board = reach_board & final_level_3 & buildable_squares;
                        let is_check = check_board.is_not_empty();

                        if is_check {
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

                if (worker_build_mask & can_dome_build_mask).is_not_empty() {
                    let new_action = AtlasMove::new_dome_build_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        worker_build_pos,
                        worker_build_height as MoveData,
                    );
                    if F & INCLUDE_SCORE != 0 {
                        let final_level_3 = exactly_level_3 & !worker_build_mask;
                        let check_board = reach_board & final_level_3 & buildable_squares;
                        let is_check = check_board.is_not_empty();

                        if is_check {
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
    }

    result
}

build_god_power!(
    build_atlas,
    god_name: GodName::Atlas,
    move_type: AtlasMove,
    move_gen: atlas_move_gen,
    hash1: 6219360493030857052,
    hash2: 4773917144301422909,
);
