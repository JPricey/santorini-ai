use crate::{
    add_scored_move,
    bitboard::BitBoard,
    board::{BoardState, FullGameState, NEIGHBOR_MAP},
    build_god_power_movers, build_power_move_generator,
    gods::{
        FullAction, GodName, GodPower, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

// from(5)|to(5)|build(5)|is_dome_build(1)
pub const ATLAS_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const ATLAS_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const ATLAS_BUILD_POSITION_OFFSET: usize = ATLAS_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const ATLAS_IS_DOME_BUILD_POSITION_OFFSET: usize = ATLAS_BUILD_POSITION_OFFSET + POSITION_WIDTH;

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
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATLAS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATLAS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ATLAS_BUILD_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_dome_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATLAS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATLAS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ATLAS_BUILD_POSITION_OFFSET)
            | ATLAS_IS_DOME_BUILD_MASK;

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
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

build_power_move_generator!(
    atlas_move_gen,
    build_winning_move: AtlasMove::new_winning_move,
    state: state,
    is_include_score: is_include_score,
    is_check: is_check,
    is_improving: is_improving,
    exactly_level_1: exactly_level_1,
    exactly_level_2: exactly_level_2,
    exactly_level_3: exactly_level_3,
    worker_start_pos: worker_start_pos,
    worker_end_pos: worker_end_pos,
    all_possible_builds: all_possible_builds,
    narrowed_builds: narrowed_builds,
    reach_board: reach_board,
    unblocked_squares: unblocked_squares,
    result: result,
    building_block: {
        for worker_build_pos in narrowed_builds {
            let worker_build_mask = BitBoard::as_mask(worker_build_pos);
            let worker_build_height = state.board.get_height(worker_build_pos);

            {
                let new_action = AtlasMove::new_basic_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );
                let is_check = {
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3 & unblocked_squares;
                    check_board.is_not_empty()
                };
                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }

            if worker_build_height < 3 {
                let new_action = AtlasMove::new_dome_build_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );
                let is_check = {
                    let final_level_3 = exactly_level_3 & !worker_build_mask;
                    let check_board = reach_board & final_level_3 & unblocked_squares;
                    check_board.is_not_empty()
                };
                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }
        }
    },
    extra_init: (),
);

pub const fn build_atlas() -> GodPower {
    god_power(
        GodName::Atlas,
        build_god_power_movers!(atlas_move_gen),
        build_god_power_actions::<AtlasMove>(),
        6219360493030857052,
        4773917144301422909,
    )
}
