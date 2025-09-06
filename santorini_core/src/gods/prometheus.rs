use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::prometheus_slide,
        move_helpers::{
            WorkerNextMoveState, build_scored_move, get_generator_prelude_state, get_sized_result,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

pub const PROMETHEUS_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const PROMETHEUS_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const PROMETHEUS_BUILD_POSITION_OFFSET: usize =
    PROMETHEUS_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const PROMETHEUS_PRE_BUILD_POSITION_OFFSET: usize =
    PROMETHEUS_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_OFFSET: usize =
    PROMETHEUS_PRE_BUILD_POSITION_OFFSET + POSITION_WIDTH;

pub const PROMETHEUS_NO_PRE_BUILD_VALUE: MoveData = 25 << PROMETHEUS_PRE_BUILD_POSITION_OFFSET;
pub const PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_VALUE: MoveData =
    1 << PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PrometheusMove(pub MoveData);

impl Into<GenericMove> for PrometheusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for PrometheusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl PrometheusMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << PROMETHEUS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << PROMETHEUS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << PROMETHEUS_BUILD_POSITION_OFFSET)
            | PROMETHEUS_NO_PRE_BUILD_VALUE;

        Self(data)
    }

    pub fn new_pre_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        pre_build_position: Square,
        is_interchangeable: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << PROMETHEUS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << PROMETHEUS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << PROMETHEUS_BUILD_POSITION_OFFSET)
            | ((pre_build_position as MoveData) << PROMETHEUS_PRE_BUILD_POSITION_OFFSET)
            | ((is_interchangeable as MoveData) << PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << PROMETHEUS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << PROMETHEUS_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> PROMETHEUS_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn pre_build_position(self) -> Option<Square> {
        let value = (self.0 >> PROMETHEUS_PRE_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn are_builds_interchangeable(self) -> bool {
        self.0 & PROMETHEUS_ARE_BUILDS_INTERCHANGEABLE_VALUE != 0
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for PrometheusMove {
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
        } else if let Some(pre_build) = self.pre_build_position() {
            write!(f, "^{} {}>{}^{}", pre_build, move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for PrometheusMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        if self.get_is_winning() {
            return vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position()),
            ]];
        }

        let build_position = self.build_position();

        if let Some(pre_build_position) = self.pre_build_position() {
            let mut res = vec![vec![
                PartialAction::Build(pre_build_position),
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position()),
                PartialAction::Build(build_position),
            ]];
            if self.are_builds_interchangeable() {
                res.push(vec![
                    PartialAction::Build(build_position),
                    PartialAction::SelectWorker(self.move_from_position()),
                    PartialAction::MoveWorker(self.move_to_position()),
                    PartialAction::Build(pre_build_position),
                ]);
            }

            res
        } else {
            vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position()),
                PartialAction::Build(build_position),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        {
            let build_position = self.build_position();
            board.build_up(build_position);
        }

        if let Some(build_position) = self.pre_build_position() {
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
        res = res * 101
            + if let Some(second_build) = self.pre_build_position() {
                let second_build_height = board.get_height(second_build);
                let su = second_build as usize;
                4 * su + second_build_height + 1
            } else {
                0
            };

        res
    }
}

pub fn prometheus_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let worker_starting_neighbors = NEIGHBOR_MAP[worker_start_pos as usize];

        let mut worker_moves = worker_starting_neighbors
            & !(prelude.board.height_map[prelude
                .board
                .get_worker_climb_height(player, worker_start_state.worker_start_height)]
                | prelude.all_workers_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, _, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                PrometheusMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers =
            (worker_start_state.other_own_workers) & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);

        let unblocked_squares = !(worker_start_state.all_non_moving_workers | prelude.domes);
        let pre_build_locations =
            worker_starting_neighbors & unblocked_squares & prelude.build_mask;
        let pre_build_worker_moves =
            worker_moves & !prelude.board.height_map[worker_start_state.worker_start_height];
        let moveable_ontop_of_prebuild = if worker_start_state.worker_start_height == 0 {
            BitBoard::EMPTY
        } else {
            pre_build_worker_moves
                & !prelude.board.height_map[worker_start_state.worker_start_height - 1]
        };

        // If we pre-build
        for pre_build_pos in pre_build_locations {
            let pre_build_mask = BitBoard::as_mask(pre_build_pos);

            let pre_build_worker_moves = pre_build_worker_moves & !pre_build_mask
                | pre_build_mask & moveable_ontop_of_prebuild;

            for mut worker_end_pos in pre_build_worker_moves.into_iter() {
                let mut worker_end_mask = BitBoard::as_mask(worker_end_pos);
                let mut worker_end_height = prelude.board.get_height(worker_end_pos)
                    + ((worker_end_pos == pre_build_pos) as usize);

                if prelude.is_against_harpies {
                    worker_end_pos = prometheus_slide(
                        &prelude.board,
                        worker_start_pos,
                        worker_end_pos,
                        worker_end_height,
                    );

                    worker_end_mask = BitBoard::as_mask(worker_end_pos);
                    worker_end_height = prelude.board.get_height(worker_end_pos)
                        + ((worker_end_pos == pre_build_pos) as usize);
                }

                let is_now_lvl_2 = (worker_end_height == 2) as usize;

                // can't use build_building_masks here due to extra logic before key squares
                let mut worker_builds = NEIGHBOR_MAP[worker_end_pos as usize] & unblocked_squares;
                let worker_plausible_next_moves = worker_builds;
                worker_builds &= prelude.build_mask;

                let both_buildable = worker_builds & pre_build_locations;
                worker_builds &= !(pre_build_mask & prelude.exactly_level_3);

                if is_interact_with_key_squares::<F>() {
                    if ((worker_end_mask | pre_build_mask) & key_squares).is_empty() {
                        worker_builds &= key_squares;
                    }
                }

                let own_final_workers = worker_start_state.other_own_workers | worker_end_mask;
                let reach_board = if prelude.is_against_hypnus
                    && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2
                {
                    BitBoard::EMPTY
                } else {
                    (other_threatening_neighbors
                        | (worker_plausible_next_moves & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                        & prelude.win_mask
                        & !own_final_workers
                };

                for worker_build_pos in worker_builds {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let is_double_build = pre_build_pos == worker_build_pos;

                    let is_either_order = !is_double_build
                        && (both_buildable | pre_build_mask | worker_build_mask) == both_buildable;

                    // avoid duplicates
                    if is_either_order && pre_build_pos > worker_build_pos {
                        continue;
                    }

                    let new_action = PrometheusMove::new_pre_build_move(
                        worker_start_pos,
                        worker_end_pos,
                        worker_build_pos,
                        pre_build_pos,
                        is_either_order,
                    );

                    let is_check = {
                        let final_level_3 = if is_double_build {
                            prelude.exactly_level_1 & pre_build_mask
                                | prelude.exactly_level_3 & !pre_build_mask
                        } else {
                            let both_build_mask = pre_build_mask | worker_build_mask;
                            prelude.exactly_level_2 & both_build_mask
                                | prelude.exactly_level_3 & !both_build_mask
                        };
                        let check_board = reach_board & final_level_3 & unblocked_squares;
                        check_board.is_not_empty()
                    };

                    result.push(build_scored_move::<F, _>(new_action, is_check, false))
                }
            }
        }

        // We dont pre-build
        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &WorkerNextMoveState {
                    other_threatening_workers,
                    other_threatening_neighbors,
                    worker_moves,
                },
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = PrometheusMove::new_basic_move(
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
        }
    }

    result
}

pub const fn build_prometheus() -> GodPower {
    god_power(
        GodName::Prometheus,
        build_god_power_movers!(prometheus_move_gen),
        build_god_power_actions::<PrometheusMove>(),
        7255800742029900355,
        11420172211286930201,
    )
}
