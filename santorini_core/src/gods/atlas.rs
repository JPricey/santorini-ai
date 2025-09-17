use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{build_scored_move, make_build_only_power_generator},
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

// from(5)|to(5)|build(5)|is_dome_build(1)
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const IS_DOME_BUILD_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

const IS_DOME_BUILD_MASK: MoveData = 1 << IS_DOME_BUILD_POSITION_OFFSET;

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
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_dome_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | IS_DOME_BUILD_MASK;

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn is_dome_build(self) -> bool {
        self.0 & IS_DOME_BUILD_MASK != 0
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
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
            PartialAction::MoveWorker(self.move_to_position().into()),
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

    fn make_move(self, board: &mut BoardState, player: Player) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
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
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_bool(self.is_dome_build());
        helper.get()
    }
}

fn atlas_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    persephone_check_result!(atlas_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    make_build_only_power_generator::<F, MUST_CLIMB, _, _, _>(
        state,
        player,
        key_squares,
        AtlasMove::new_winning_move,
        |context| {
            for worker_build_pos in context.worker_next_build_state.narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let worker_build_height = state.board.get_height(worker_build_pos);

                {
                    let new_action = AtlasMove::new_basic_move(
                        context.worker_start_state.worker_start_pos,
                        context.worker_end_state.worker_end_pos,
                        worker_build_pos,
                    );
                    let is_check = {
                        let final_level_3 = (context.prelude.exactly_level_2 & worker_build_mask)
                            | (context.prelude.exactly_level_3 & !worker_build_mask);
                        let check_board = context.reach_board & final_level_3;
                        check_board.is_not_empty()
                    };

                    context.result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        context.worker_end_state.is_improving,
                    ))
                }

                if worker_build_height < 3 {
                    let new_action = AtlasMove::new_dome_build_move(
                        context.worker_start_state.worker_start_pos,
                        context.worker_end_state.worker_end_pos,
                        worker_build_pos,
                    );
                    let is_check = {
                        let final_level_3 = context.prelude.exactly_level_3 & !worker_build_mask;
                        let check_board = context.reach_board & final_level_3;
                        check_board.is_not_empty()
                    };

                    context.result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        context.worker_end_state.is_improving,
                    ))
                }
            }
        },
    )
}

pub const fn build_atlas() -> GodPower {
    god_power(
        GodName::Atlas,
        build_god_power_movers!(atlas_move_gen),
        build_god_power_actions::<AtlasMove>(),
        6219360493030857052,
        4773917144301422909,
    )
}
