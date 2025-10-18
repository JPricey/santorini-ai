use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves,
        },
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
    let mut result = persephone_check_result!(atlas_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, AtlasMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                AtlasMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);

            let all_possible_builds_no_limus =
                NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize] & unblocked_squares;
            let mut narrowed_builds_no_limus = all_possible_builds_no_limus;

            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (worker_end_move_state.worker_end_mask
                    & prelude.key_squares)
                    .is_not_empty() as usize;
                narrowed_builds_no_limus &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            let regular_buildables = narrowed_builds_no_limus & prelude.build_mask;
            let dome_buildables = narrowed_builds_no_limus & !prelude.exactly_level_3;

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            for worker_build_pos in regular_buildables {
                let new_action = AtlasMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
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

            for worker_dome_pos in dome_buildables {
                let new_action = AtlasMove::new_dome_build_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_dome_pos,
                );
                let is_check = {
                    let final_level_3 =
                        prelude.exactly_level_3 & !BitBoard::as_mask(worker_dome_pos);
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

pub const fn build_atlas() -> GodPower {
    god_power(
        GodName::Atlas,
        build_god_power_movers!(atlas_move_gen),
        build_god_power_actions::<AtlasMove>(),
        6219360493030857052,
        4773917144301422909,
    )
}
