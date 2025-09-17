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

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const SECOND_BUILD_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const NO_SECOND_BUILD_VALUE: MoveData = 25 << SECOND_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct DemeterMove(pub MoveData);

impl Into<GenericMove> for DemeterMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for DemeterMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl DemeterMove {
    fn new_demeter_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | NO_SECOND_BUILD_VALUE;

        Self(data)
    }

    fn new_demeter_two_build_move(
        move_from_position: Square,
        move_to_position: Square,
        mut build_position: Square,
        mut build_position_2: Square,
    ) -> Self {
        if build_position_2 < build_position {
            std::mem::swap(&mut build_position, &mut build_position_2);
        }

        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((build_position_2 as MoveData) << SECOND_BUILD_POSITION_OFFSET);

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

    fn second_build_position(self) -> Option<Square> {
        let value = (self.0 >> SECOND_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for DemeterMove {
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
        } else if let Some(second_build) = self.second_build_position() {
            write!(f, "{}>{}^{}^{}", move_from, move_to, build, second_build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for DemeterMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut move_vec = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![move_vec];
        }

        let build_position = self.build_position();
        if let Some(second_build) = self.second_build_position() {
            let mut mirror = move_vec.clone();
            move_vec.push(PartialAction::Build(build_position));
            move_vec.push(PartialAction::Build(second_build));

            mirror.push(PartialAction::Build(second_build));
            mirror.push(PartialAction::Build(build_position));
            vec![move_vec, mirror]
        } else {
            move_vec.push(PartialAction::Build(build_position));
            vec![move_vec]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);
        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
        if let Some(build_position) = self.second_build_position() {
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
        helper.add_maybe_square_with_height(board, self.second_build_position());
        helper.get()
    }
}

fn demeter_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    persephone_check_result!(demeter_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    make_build_only_power_generator::<F, MUST_CLIMB, _, _, _>(
        state,
        player,
        key_squares,
        DemeterMove::new_winning_move,
        |context| {
            let mut second_builds = context.worker_next_build_state.all_possible_builds;
            for worker_build_pos in context.worker_next_build_state.narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                second_builds ^= worker_build_mask;

                {
                    let new_action = DemeterMove::new_demeter_move(
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

                for second_worker_build_pos in second_builds {
                    let second_worker_build_mask = BitBoard::as_mask(second_worker_build_pos);
                    let total_build_mask = worker_build_mask | second_worker_build_mask;

                    let new_action = DemeterMove::new_demeter_two_build_move(
                        context.worker_start_state.worker_start_pos,
                        context.worker_end_state.worker_end_pos,
                        worker_build_pos,
                        second_worker_build_pos,
                    );

                    let is_check = {
                        let final_level_3 = (context.prelude.exactly_level_2 & total_build_mask)
                            | (context.prelude.exactly_level_3 & !total_build_mask);
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

pub const fn build_demeter() -> GodPower {
    god_power(
        GodName::Demeter,
        build_god_power_movers!(demeter_move_gen),
        build_god_power_actions::<DemeterMove>(),
        12982186464139786854,
        3782535430861395331,
    )
}
