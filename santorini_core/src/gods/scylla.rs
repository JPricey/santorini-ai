use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_basic_moves_from_raw_data_with_custom_blockers_no_affinity,
            get_generator_prelude_state, get_standard_reach_board_from_parts,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves, restrict_moves_by_affinity_area,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const SCYLLA_MOVE_FROM_POSITION_OFFSET: usize = 0;
const SCYLLA_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const SCYLLA_BUILD_POSITION_OFFSET: usize = SCYLLA_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const SCYLLA_DRAG_FROM_POSITION_OFFSET: usize = SCYLLA_BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct ScyllaMoveMove(pub MoveData);

impl Into<GenericMove> for ScyllaMoveMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ScyllaMoveMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ScyllaMoveMove {
    fn new_scylla_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << SCYLLA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << SCYLLA_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << SCYLLA_BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << SCYLLA_DRAG_FROM_POSITION_OFFSET);

        Self(data)
    }

    fn new_scylla_drag_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        drag_from_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << SCYLLA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << SCYLLA_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << SCYLLA_BUILD_POSITION_OFFSET)
            | ((drag_from_position as MoveData) << SCYLLA_DRAG_FROM_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << SCYLLA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << SCYLLA_MOVE_TO_POSITION_OFFSET)
            | ((25 as MoveData) << SCYLLA_DRAG_FROM_POSITION_OFFSET)
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
        Square::from((self.0 >> SCYLLA_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn maybe_drag_from_position(&self) -> Option<Square> {
        let value = (self.0 >> SCYLLA_DRAG_FROM_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ScyllaMoveMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            if let Some(drag_from) = self.maybe_drag_from_position() {
                let drag_to = self.move_from_position();
                write!(f, "({}>{}){}>{}#", drag_from, drag_to, move_from, move_to)
            } else {
                write!(f, "{}>{}#", move_from, move_to)
            }
        } else {
            if let Some(drag_from) = self.maybe_drag_from_position() {
                let drag_to = self.move_from_position();
                write!(
                    f,
                    "({}>{}){}>{}^{}",
                    drag_from, drag_to, move_from, move_to, build
                )
            } else {
                write!(f, "{}>{}^{}", move_from, move_to, build)
            }
        }
    }
}

impl GodMove for ScyllaMoveMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut result = vec![];
        let move_from_position = self.move_from_position();

        result.push(PartialAction::SelectWorker(move_from_position));
        result.push(PartialAction::MoveWorker(self.move_to_position().into()));

        if let Some(drag_from) = self.maybe_drag_from_position() {
            result.push(PartialAction::ForceOpponentWorker(
                drag_from,
                move_from_position,
            ));
        }

        if !self.get_is_winning() {
            result.push(PartialAction::Build(self.build_position()));
        }

        return vec![result];
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        let move_from = BitBoard::as_mask(self.move_from_position());
        let move_to = BitBoard::as_mask(self.move_to_position());
        board.worker_xor(player, move_to ^ move_from);

        if let Some(drag_from) = self.maybe_drag_from_position() {
            board.oppo_worker_xor(other_god, !player, drag_from.to_board() ^ move_from);
        }

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_maybe_square_with_height(board, self.maybe_drag_from_position());
        helper.get()
    }
}

pub(super) fn scylla_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(scylla_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let blocked_squares = prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut worker_moves_no_affinity_restriction =
            get_basic_moves_from_raw_data_with_custom_blockers_no_affinity::<MUST_CLIMB>(
                &prelude,
                worker_start_state.worker_start_pos,
                worker_start_state.worker_start_height,
                blocked_squares,
            );

        // We don't care about affinity restriction for winning moves
        // against aphrodite we'd always be able to drag her into range so whatever
        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_moves_no_affinity_restriction & prelude.exactly_level_3 & prelude.win_mask;

            if push_winning_moves::<F, ScyllaMoveMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                ScyllaMoveMove::new_winning_move,
            ) {
                return result;
            }

            worker_moves_no_affinity_restriction ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers = worker_start_state.other_own_workers & checkable_mask;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &prelude.standard_neighbor_map);

        let no_drag_worker_moves = restrict_moves_by_affinity_area(
            worker_start_state.worker_start_mask,
            worker_moves_no_affinity_restriction,
            prelude.affinity_area,
        );

        for worker_move_pos in no_drag_worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_move_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = get_standard_reach_board_from_parts::<F>(
                &prelude,
                other_threatening_workers,
                other_threatening_neighbors,
                worker_end_move_state.worker_end_pos,
                worker_end_move_state.is_now_lvl_2,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = ScyllaMoveMove::new_scylla_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );

                let build_pos_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_pos_mask)
                        | (prelude.exactly_level_3 & !build_pos_mask);
                    reach_board & final_level_3
                }
                .is_not_empty();

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }

        let possible_drags = NEIGHBOR_MAP[worker_start_pos as usize] & prelude.oppo_workers;
        for dragged_worker_pos in possible_drags {
            let dragged_worker_from_mask = dragged_worker_pos.to_board();
            let new_oppo_workers = prelude.oppo_workers
                ^ dragged_worker_from_mask
                ^ worker_start_state.worker_start_mask;

            let all_blockers_after_drag =
                prelude.domes_and_frozen | new_oppo_workers | worker_start_state.other_own_workers;

            let new_build_mask =
                prelude.other_god.get_build_mask(new_oppo_workers) | prelude.exactly_level_3;

            for worker_move_pos in worker_moves_no_affinity_restriction {
                let worker_end_move_state =
                    get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_move_pos);

                let all_possible_builds = NEIGHBOR_MAP
                    [worker_end_move_state.worker_end_pos as usize]
                    & !(all_blockers_after_drag)
                    & new_build_mask;

                let mut narrowed_builds = all_possible_builds;
                if is_interact_with_key_squares::<F>() {
                    let interact_board = key_squares
                        & (worker_end_move_state.worker_end_mask
                            | dragged_worker_from_mask
                            | worker_start_state.worker_start_mask);

                    if interact_board.is_empty() {
                        narrowed_builds &= prelude.key_squares;
                    }
                }

                let reach_board = get_standard_reach_board_from_parts::<F>(
                    &prelude,
                    other_threatening_workers,
                    other_threatening_neighbors,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.is_now_lvl_2,
                    !(all_blockers_after_drag | worker_end_move_state.worker_end_mask),
                );

                for worker_build_pos in narrowed_builds {
                    let new_action = ScyllaMoveMove::new_scylla_drag_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                        dragged_worker_pos,
                    );
                    let build_pos_mask = worker_build_pos.to_board();

                    let is_check = {
                        let final_level_3 = (prelude.exactly_level_2 & build_pos_mask)
                            | (prelude.exactly_level_3 & !build_pos_mask);
                        let check_board = reach_board & final_level_3;
                        check_board.is_not_empty()
                    };

                    result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        worker_end_move_state.is_improving,
                    ));
                }
            }
        }
    }

    result
}

pub const fn build_scylla() -> GodPower {
    god_power(
        GodName::Scylla,
        build_god_power_movers!(scylla_move_gen),
        build_god_power_actions::<ScyllaMoveMove>(),
        12345678901234567890,
        9876543210987654321,
    )
    .with_nnue_god_name(GodName::Apollo)
}
