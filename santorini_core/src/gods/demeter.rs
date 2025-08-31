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

pub const DEMETER_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const DEMETER_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const DEMETER_BUILD_POSITION_OFFSET: usize = DEMETER_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const DEMETER_SECOND_BUILD_POSITION_OFFSET: usize =
    DEMETER_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const DEMETER_NO_SECOND_BUILD_VALUE: MoveData = 25 << DEMETER_SECOND_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct DemeterMove(pub MoveData);

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
    pub fn new_demeter_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << DEMETER_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << DEMETER_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << DEMETER_BUILD_POSITION_OFFSET)
            | DEMETER_NO_SECOND_BUILD_VALUE;

        Self(data)
    }

    pub fn new_demeter_two_build_move(
        move_from_position: Square,
        move_to_position: Square,
        mut build_position: Square,
        mut build_position_2: Square,
    ) -> Self {
        if build_position_2 < build_position {
            std::mem::swap(&mut build_position, &mut build_position_2);
        }

        let data: MoveData = ((move_from_position as MoveData)
            << DEMETER_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << DEMETER_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << DEMETER_BUILD_POSITION_OFFSET)
            | ((build_position_2 as MoveData) << DEMETER_SECOND_BUILD_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << DEMETER_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << DEMETER_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> DEMETER_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn second_build_position(self) -> Option<Square> {
        let value = (self.0 >> DEMETER_SECOND_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
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
            PartialAction::MoveWorker(self.move_to_position()),
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

    fn make_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);
        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        board.build_up(self.build_position());
        if let Some(build_position) = self.second_build_position() {
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

        board.unbuild(self.build_position());
        if let Some(build_position) = self.second_build_position() {
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
        res = res * 101
            + if let Some(second_build) = self.second_build_position() {
                let second_build_height = board.get_height(second_build);
                let su = second_build as usize;
                4 * su + second_build_height + 1
            } else {
                0
            };

        res
    }
}

build_power_move_generator!(
    demeter_move_gen,
    build_winning_move: DemeterMove::new_winning_move,
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
        let mut second_builds = all_possible_builds;
        for worker_build_pos in narrowed_builds {
            let worker_build_mask = BitBoard::as_mask(worker_build_pos);
            second_builds ^= worker_build_mask;

            {
                let new_action = DemeterMove::new_demeter_move(
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

            for second_worker_build_pos in second_builds {
                let second_worker_build_mask = BitBoard::as_mask(second_worker_build_pos);
                let total_build_mask = worker_build_mask | second_worker_build_mask;

                let new_action = DemeterMove::new_demeter_two_build_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                    second_worker_build_pos,
                );

                let is_check = {
                    let final_level_3 = (exactly_level_2 & total_build_mask)
                        | (exactly_level_3 & !total_build_mask);
                    let check_board = reach_board & final_level_3 & unblocked_squares;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }
        }
    },
    extra_init: (),
);

pub const fn build_demeter() -> GodPower {
    god_power(
        GodName::Demeter,
        build_god_power_movers!(demeter_move_gen),
        build_god_power_actions::<DemeterMove>(),
        12982186464139786854,
        3782535430861395331,
    )
}
