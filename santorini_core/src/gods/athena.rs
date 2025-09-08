use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::slide_position,
        move_helpers::{
            WorkerEndMoveState, build_scored_move, get_generator_prelude_state, 
            get_standard_reach_board, get_worker_next_build_state_with_is_matched,
            get_worker_next_move_state, get_worker_start_move_state, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

pub const ATHENA_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const ATHENA_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const ATHENA_BUILD_POSITION_OFFSET: usize = ATHENA_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const ATHENA_DID_IMPROVE_OFFSET: usize = ATHENA_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const ATHENA_DID_IMPROVE_CHANGE_OFFSET: usize = ATHENA_DID_IMPROVE_OFFSET + 1;

pub const ATHENA_DID_IMPROVE_MASK: MoveData = 1 << ATHENA_DID_IMPROVE_OFFSET;
pub const ATHENA_DID_IMPROVE_CHANGE_MASK: MoveData = 1 << ATHENA_DID_IMPROVE_CHANGE_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AthenaMove(pub MoveData);

impl Into<GenericMove> for AthenaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AthenaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AthenaMove {
    pub fn new_athena_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        did_climb: bool,
        did_climb_change: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATHENA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATHENA_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ATHENA_BUILD_POSITION_OFFSET)
            | ((did_climb) as MoveData) << ATHENA_DID_IMPROVE_OFFSET
            | ((did_climb_change) as MoveData) << ATHENA_DID_IMPROVE_CHANGE_OFFSET;

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATHENA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATHENA_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> ATHENA_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }

    pub fn get_did_climb(&self) -> bool {
        (self.0 & ATHENA_DID_IMPROVE_MASK) != 0
    }

    pub fn get_did_climb_change(&self) -> bool {
        (self.0 & ATHENA_DID_IMPROVE_CHANGE_MASK) != 0
    }
}

impl std::fmt::Debug for AthenaMove {
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
        } else if self.get_did_climb() {
            write!(f, "{}>{}!^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for AthenaMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        if self.get_is_winning() {
            return vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position()),
            ]];
        }

        let build_position = self.build_position();
        vec![vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position()),
            PartialAction::Build(build_position),
        ]]
    }

    fn make_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);
        board.flip_worker_can_climb(!board.current_player, self.get_did_climb_change())
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

        res
    }
}

pub(super) fn athena_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(athena_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let did_not_improve_last_turn = prelude.board.get_worker_can_climb(!player);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, AthenaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                AthenaMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if F & super::generic::MATE_ONLY != 0 {
            continue;
        }

        for mut worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_height;
            let is_improving;
            let is_improving_for_power;
            if prelude.is_against_harpies {
                is_improving_for_power = prelude.board.get_height(worker_end_pos)
                    > worker_start_state.worker_start_height;
                worker_end_pos = slide_position(
                    prelude.board,
                    worker_start_state.worker_start_pos,
                    worker_end_pos,
                );

                worker_end_height = prelude.board.get_height(worker_end_pos);
                is_improving = worker_end_height > worker_start_state.worker_start_height;
            } else {
                worker_end_height = prelude.board.get_height(worker_end_pos);
                is_improving = worker_end_height > worker_start_state.worker_start_height;
                is_improving_for_power = is_improving;
            }

            let worker_end_move_state = WorkerEndMoveState {
                worker_end_pos,
                worker_end_height,
                is_improving,
                worker_end_mask: BitBoard::as_mask(worker_end_pos),
                is_now_lvl_2: (worker_end_height == 2) as u32,
            };

            let worker_next_build_state = get_worker_next_build_state_with_is_matched::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
                is_improving_for_power
                    || (worker_end_move_state.worker_end_mask & key_squares).is_not_empty(),
            );

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = AthenaMove::new_athena_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                    is_improving_for_power,
                    is_improving_for_power == did_not_improve_last_turn,
                );
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos));
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    result
}

pub const fn build_athena() -> GodPower {
    god_power(
        GodName::Athena,
        build_god_power_movers!(athena_move_gen),
        build_god_power_actions::<AthenaMove>(),
        1867170053174999423,
        15381411414297507361,
    )
}
