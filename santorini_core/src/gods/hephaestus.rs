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

const HEPH_MOVE_FROM_POSITION_OFFSET: usize = 0;
const HEPH_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const HEPH_BUILD_POSITION_OFFSET: usize = POSITION_WIDTH * 2;
const HEPH_IS_DOUBLE_BUILD_POSITION_OFFSET: usize = POSITION_WIDTH * 3;

const HEPH_IS_DOUBLE_BUILD_MASK: MoveData = 1 << HEPH_IS_DOUBLE_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
struct HephMove(pub MoveData);

impl Into<GenericMove> for HephMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for HephMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl HephMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HEPH_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HEPH_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << HEPH_BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_double_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HEPH_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HEPH_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << HEPH_BUILD_POSITION_OFFSET)
            | HEPH_IS_DOUBLE_BUILD_MASK;

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HEPH_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HEPH_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> HEPH_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn is_double_build(self) -> bool {
        self.0 & HEPH_IS_DOUBLE_BUILD_MASK != 0
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl GodMove for HephMove {
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

        if self.is_double_build() {
            res.push(PartialAction::Build(build_position));
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
        if self.is_double_build() {
            board.double_build_up(build_position);
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
        helper.add_bool(self.is_double_build());
        helper.get()
    }
}

impl std::fmt::Debug for HephMove {
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
        } else if self.is_double_build() {
            write!(f, "{}>{}^^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

fn hephaestus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    persephone_check_result!(hephaestus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    make_build_only_power_generator::<F, MUST_CLIMB, _, _, _>(
        state,
        player,
        key_squares,
        HephMove::new_winning_move,
        |context| {
            for worker_build_pos in context.worker_next_build_state.narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);

                {
                    let new_action = HephMove::new_basic_move(
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

                if context.prelude.board.get_height(worker_build_pos) < 2 {
                    let new_action = HephMove::new_double_build_move(
                        context.worker_start_state.worker_start_pos,
                        context.worker_end_state.worker_end_pos,
                        worker_build_pos,
                    );
                    let is_check = {
                        let final_level_3 = (context.prelude.exactly_level_1 & worker_build_mask)
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
            }
        },
    )
}

pub const fn build_hephaestus() -> GodPower {
    god_power(
        GodName::Hephaestus,
        build_god_power_movers!(hephaestus_move_gen),
        build_god_power_actions::<HephMove>(),
        8778550832748251380,
        14400518822473574269,
    )
}
