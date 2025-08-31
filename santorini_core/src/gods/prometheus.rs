use crate::{
    add_scored_move,
    bitboard::BitBoard,
    board::{BoardState, FullGameState, NEIGHBOR_MAP},
    build_god_power_movers, build_parse_flags, build_push_winning_moves,
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
    variable_prelude,
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

    fn unmake_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.unset_winner(board.current_player);
            return;
        }

        {
            let build_position = self.build_position();
            board.unbuild(build_position);
        }

        if let Some(build_position) = self.pre_build_position() {
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

fn prometheus_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    build_parse_flags!(
        is_mate_only,
        is_include_score,
        is_stop_on_mate,
        is_interact_with_key_squares
    );

    variable_prelude!(
        state,
        player,
        board,
        other_player,
        current_player_idx,
        other_player_idx,
        other_god,
        exactly_level_0,
        exactly_level_1,
        exactly_level_2,
        exactly_level_3,
        win_mask,
        domes,
        own_workers,
        other_workers,
        result,
        all_workers_mask,
        is_mate_only,
        current_workers,
        checkable_worker_positions_mask,
    );

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);
        let other_own_workers = own_workers ^ moving_worker_start_mask;

        let mut other_threatening_neighbors = BitBoard::EMPTY;
        if is_include_score {
            let other_checkable_workers =
                (current_workers ^ moving_worker_start_mask) & exactly_level_2;
            for other_pos in other_checkable_workers {
                other_threatening_neighbors |= NEIGHBOR_MAP[other_pos as usize];
            }
        }

        let worker_starting_neighbors = NEIGHBOR_MAP[moving_worker_start_pos as usize];

        let mut worker_moves = worker_starting_neighbors
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if is_mate_only || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & exactly_level_3 & win_mask;
            build_push_winning_moves!(
                moves_to_level_3,
                worker_moves,
                PrometheusMove::new_winning_move,
                moving_worker_start_pos,
                result,
                is_stop_on_mate,
            );
        }

        if is_mate_only {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let unblocked_squares = !(non_selected_workers | board.height_map[3]);

        let pre_build_locations = worker_starting_neighbors & unblocked_squares;
        let pre_build_worker_moves = worker_moves & !board.height_map[worker_starting_height];
        let moveable_ontop_of_prebuild = if worker_starting_height == 0 {
            BitBoard::EMPTY
        } else {
            pre_build_worker_moves & !board.height_map[worker_starting_height - 1]
        };

        // If we pre-build
        for pre_build_pos in pre_build_locations {
            let pre_build_mask = BitBoard::as_mask(pre_build_pos);

            let pre_build_worker_moves = pre_build_worker_moves & !pre_build_mask
                | pre_build_mask & moveable_ontop_of_prebuild;

            for moving_worker_end_pos in pre_build_worker_moves.into_iter() {
                let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
                let worker_end_height = board.get_height(moving_worker_end_pos)
                    + ((moving_worker_end_pos == pre_build_pos) as usize);
                let is_improving = worker_end_height > worker_starting_height;

                let mut worker_builds =
                    NEIGHBOR_MAP[moving_worker_end_pos as usize] & unblocked_squares;
                let worker_plausible_next_moves = worker_builds;

                let both_buildable = worker_builds & pre_build_locations;
                worker_builds &= !(pre_build_mask & exactly_level_3);

                if is_interact_with_key_squares {
                    if ((moving_worker_end_mask | pre_build_mask) & key_squares).is_empty() {
                        worker_builds = worker_builds & key_squares;
                    }
                }

                let own_final_workers = other_own_workers | moving_worker_end_mask;
                let reach_board = (other_threatening_neighbors
                    | (worker_plausible_next_moves
                        & BitBoard::CONDITIONAL_MASK[(worker_end_height == 2) as usize]))
                    & win_mask
                    & !own_final_workers;

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
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        worker_build_pos,
                        pre_build_pos,
                        is_either_order,
                    );

                    let is_check = {
                        let final_level_3 = if is_double_build {
                            exactly_level_1 & pre_build_mask | exactly_level_3 & !pre_build_mask
                        } else {
                            let both_build_mask = pre_build_mask | worker_build_mask;
                            exactly_level_2 & both_build_mask | exactly_level_3 & !both_build_mask
                        };
                        let check_board = reach_board & final_level_3 & unblocked_squares;
                        check_board.is_not_empty()
                    };

                    add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
                }
            }
        }

        // If we don't pre-build
        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);
            let is_improving = worker_end_height > worker_starting_height;

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & unblocked_squares;
            let worker_plausible_next_moves = worker_builds;

            if is_interact_with_key_squares {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let own_final_workers = other_own_workers | moving_worker_end_mask;
            let reach_board = (other_threatening_neighbors
                | (worker_plausible_next_moves
                    & BitBoard::CONDITIONAL_MASK[(worker_end_height == 2) as usize]))
                & win_mask
                & !own_final_workers;

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = PrometheusMove::new_basic_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
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
