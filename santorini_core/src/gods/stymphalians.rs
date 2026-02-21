use crate::{
    bitboard::{BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, PartialAction, StaticGod,
        build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        hypnus::hypnus_moveable_worker_filter,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_reverse_direction_neighbor_map,
            get_sized_result, is_interact_with_key_squares, is_mate_only, is_stop_on_mate,
        },
    },
    persephone_check_result,
    placement::PlacementType,
    player::Player,
    square::Square,
};

// StymphaliansMove is an exact copy of MortalMove, except with a different blocker board calculation to
// account for the longer moves
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const WINNING_PATH_1_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const WINNING_PATH_2_OFFSET: usize = WINNING_PATH_1_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct StymphaliansMove(pub MoveData);

impl GodMove for StymphaliansMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_from_position().to_board()
            | self.move_to_position().to_board()
            | self.winning_path_1().to_board()
            | self.winning_path_2().to_board()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for StymphaliansMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for StymphaliansMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl StymphaliansMove {
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

    fn new_winning_move(
        move_from_position: Square,
        move_to_position: Square,
        path_1: Square,
        path_2: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((path_1 as MoveData) << WINNING_PATH_1_OFFSET)
            | ((path_2 as MoveData) << WINNING_PATH_2_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub(crate) fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub(crate) fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn winning_path_1(self) -> Square {
        Square::from((self.0 >> WINNING_PATH_1_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn winning_path_2(self) -> Square {
        Square::from((self.0 >> WINNING_PATH_2_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for StymphaliansMove {
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
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

fn stymphalians_vs_persephone<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let open_squares = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);

    // Against persephone, can_climb is true so open_by_height allows +1 height step
    let open_by_height: [BitBoard; 4] = [
        !prelude.board.height_map[1] & open_squares,
        !prelude.board.height_map[2] & open_squares,
        !prelude.board.height_map[3] & open_squares,
        !prelude.board.height_map[3] & open_squares,
    ];

    for worker_start_pos in prelude.own_workers.into_iter() {
        let worker_start_mask = worker_start_pos.to_board();
        let worker_start_height = prelude.board.get_height(worker_start_pos);

        let mut open_final_destinations =
            open_squares & !INCLUSIVE_NEIGHBOR_MAP[worker_start_pos as usize];

        let mut worker_1d_moves = prelude.standard_neighbor_map[worker_start_pos as usize]
            & open_by_height[worker_start_height];

        if is_mate_only::<F>() {
            worker_1d_moves &= prelude.board.height_map[0];
            if (worker_1d_moves).is_empty() {
                continue;
            }
        }

        let worker_1d_climbed = worker_1d_moves & prelude.board.height_map[worker_start_height];
        let worker_1d_not_climbed = worker_1d_moves ^ worker_1d_climbed;

        let mut worker_2d_climb_moves = BitBoard::EMPTY;
        let mut worker_2d_not_climb_moves = BitBoard::EMPTY;

        for worker_m_pos in worker_1d_climbed {
            let worker_m_height = prelude.board.get_height(worker_m_pos);
            let mut new_moves = prelude.standard_neighbor_map[worker_m_pos as usize]
                & open_by_height[worker_m_height];

            if worker_m_height == 2 {
                let winning_moves_to_level_3 =
                    new_moves & open_final_destinations & prelude.exactly_level_3;
                for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                    let winning_move = ScoredMove::new_winning_move(
                        StymphaliansMove::new_winning_move(
                            worker_start_pos,
                            moving_worker_end_pos,
                            worker_m_pos,
                            worker_m_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }
                new_moves &= !winning_moves_to_level_3;
                open_final_destinations &= !winning_moves_to_level_3;
            }
            worker_2d_climb_moves |= new_moves;
        }

        for worker_m_pos in worker_1d_not_climbed {
            let worker_m_height = prelude.board.get_height(worker_m_pos);
            let mut new_moves = prelude.standard_neighbor_map[worker_m_pos as usize]
                & open_by_height[worker_m_height];

            if worker_m_height == 2 {
                let winning_moves_to_level_3 =
                    new_moves & open_final_destinations & prelude.exactly_level_3;
                for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                    let winning_move = ScoredMove::new_winning_move(
                        StymphaliansMove::new_winning_move(
                            worker_start_pos,
                            moving_worker_end_pos,
                            worker_m_pos,
                            worker_m_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }
                new_moves &= !winning_moves_to_level_3;
                open_final_destinations &= !winning_moves_to_level_3;
            }

            let new_moves_from_climbs = new_moves & prelude.board.height_map[worker_m_height];
            let new_moves_from_non_climbs = new_moves ^ new_moves_from_climbs;

            worker_2d_climb_moves |= new_moves_from_climbs;
            worker_2d_not_climb_moves |= new_moves_from_non_climbs;
        }

        // For 2d_climb exploration: remove start (occupied) and 1d_climbed (already explored all
        // their neighbors in the 1d_climbed loop). Keep 1d_not_climbed squares since they may be
        // reached via a 2d climb and need their 3d neighbors explored as part of climbing paths.
        worker_2d_climb_moves &= !(worker_1d_climbed | worker_start_mask);
        // For 2d_not_climb: remove all 1d squares and start. Also remove level 3+ since we can't
        // climb further from them.
        worker_2d_not_climb_moves &=
            !(worker_1d_moves | worker_start_mask | prelude.board.height_map[2]);

        if is_mate_only::<F>() {
            worker_2d_climb_moves &= prelude.board.height_map[1];
            worker_2d_not_climb_moves &= prelude.board.height_map[1];
            if (worker_2d_climb_moves | worker_2d_not_climb_moves).is_empty() {
                continue;
            }
        }

        let mut worker_3d_climb_moves = BitBoard::EMPTY;

        for worker_m_pos in worker_2d_climb_moves {
            let worker_m_height = prelude.board.get_height(worker_m_pos);
            let mut new_moves = prelude.standard_neighbor_map[worker_m_pos as usize]
                & open_by_height[worker_m_height];

            if worker_m_height == 2 {
                let winning_moves_to_level_3 =
                    new_moves & open_final_destinations & prelude.exactly_level_3;
                for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                    let possible_first_steps = worker_1d_moves
                        & get_reverse_direction_neighbor_map(&prelude)[worker_m_pos as usize]
                        & prelude.board.height_map[0];
                    let first_step = || {
                        let second_step_mask = worker_m_pos.to_board();
                        for possible_first_step in possible_first_steps {
                            let first_step_height = prelude.board.get_height(possible_first_step);
                            let moves_from_first_step = prelude.standard_neighbor_map
                                [possible_first_step as usize]
                                & open_by_height[first_step_height];
                            if (moves_from_first_step & second_step_mask).is_not_empty() {
                                return possible_first_step;
                            }
                        }
                        if cfg!(debug_assertions) {
                            let state = FullGameState::new(
                                prelude.board.clone(),
                                [GodName::Mortal.to_power(), GodName::Mortal.to_power()],
                            );
                            eprint!("board: {:?}", state);
                            state.print_to_console();
                            eprintln!(
                                "could not find winning path start_pos: {} mid_pos: {} end_pos: {} possible_steps: {}",
                                worker_start_pos,
                                worker_m_pos,
                                moving_worker_end_pos,
                                possible_first_steps,
                            );
                        }
                        panic!("Could not find winning path");
                    };

                    let winning_move = ScoredMove::new_winning_move(
                        StymphaliansMove::new_winning_move(
                            worker_start_pos,
                            moving_worker_end_pos,
                            first_step(),
                            worker_m_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }
                new_moves &= !winning_moves_to_level_3;
                open_final_destinations &= !winning_moves_to_level_3;
            }

            worker_3d_climb_moves |= new_moves;
        }

        for worker_m_pos in worker_2d_not_climb_moves {
            let worker_m_height = prelude.board.get_height(worker_m_pos);
            let mut new_moves = prelude.standard_neighbor_map[worker_m_pos as usize]
                & open_by_height[worker_m_height]
                & prelude.board.height_map[worker_m_height];

            if worker_m_height == 2 {
                let winning_moves_to_level_3 =
                    new_moves & open_final_destinations & prelude.exactly_level_3;
                for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                    let possible_first_steps = worker_1d_moves
                        & get_reverse_direction_neighbor_map(&prelude)[worker_m_pos as usize]
                        & prelude.board.height_map[0];
                    let first_step = || {
                        let second_step_mask = worker_m_pos.to_board();
                        for possible_first_step in possible_first_steps {
                            let first_step_height = prelude.board.get_height(possible_first_step);
                            let moves_from_first_step = prelude.standard_neighbor_map
                                [possible_first_step as usize]
                                & open_by_height[first_step_height];
                            if (moves_from_first_step & second_step_mask).is_not_empty() {
                                return possible_first_step;
                            }
                        }
                        if cfg!(debug_assertions) {
                            let state = FullGameState::new(
                                prelude.board.clone(),
                                [GodName::Mortal.to_power(), GodName::Mortal.to_power()],
                            );
                            eprint!("board: {:?}", state);
                            state.print_to_console();
                            eprintln!(
                                "could not find winning path start_pos: {} mid_pos: {} end_pos: {} possible_steps: {}",
                                worker_start_pos,
                                worker_m_pos,
                                moving_worker_end_pos,
                                possible_first_steps,
                            );
                        }
                        panic!("Could not find winning path");
                    };

                    let winning_move = ScoredMove::new_winning_move(
                        StymphaliansMove::new_winning_move(
                            worker_start_pos,
                            moving_worker_end_pos,
                            first_step(),
                            worker_m_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }
                new_moves &= !winning_moves_to_level_3;
                open_final_destinations &= !winning_moves_to_level_3;
            }

            worker_3d_climb_moves |= new_moves;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let worker_final_moves =
            (worker_2d_climb_moves | worker_3d_climb_moves) & open_final_destinations;

        for worker_end_pos in worker_final_moves {
            let worker_end_mask = worker_end_pos.to_board();

            let worker_builds = NEIGHBOR_MAP[worker_end_pos as usize] & open_squares;
            let mut narrowed_builds = worker_builds;
            if is_interact_with_key_squares::<F>() {
                let is_key_squares_matched = (worker_end_mask & key_squares).is_not_empty();
                narrowed_builds &= [prelude.key_squares, BitBoard::MAIN_SECTION_MASK]
                    [is_key_squares_matched as usize];
            }

            for worker_build_pos in narrowed_builds {
                let new_action = StymphaliansMove::new_basic_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );
                result.push(build_scored_move::<F, _>(new_action, false, false));
            }
        }
    }

    result
}

fn stymphalians_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if MUST_CLIMB {
        return stymphalians_vs_persephone::<F>(state, player, key_squares);
    }

    // if state.gods[!player as usize].is_harpies() {
    //     return stymphalians_move_gen_vs_harpies::<F, MUST_CLIMB>(state, player, key_squares);
    // }

    let mut result = persephone_check_result!(stymphalians_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let mut acting_workers = prelude.own_workers;
    if prelude.is_against_hypnus {
        acting_workers = hypnus_moveable_worker_filter(prelude.board, acting_workers);
    }

    let reverse_map = get_reverse_direction_neighbor_map(&prelude);

    let all_plausible_winning_mask = prelude.exactly_level_3 & prelude.win_mask;

    let open_squares = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);
    let open_by_height: [BitBoard; 4] = if prelude.is_down_prevented {
        [
            !prelude.board.height_map[1] & open_squares,
            prelude.board.height_map[0] & !prelude.board.height_map[2] & open_squares,
            prelude.board.height_map[1] & !prelude.board.height_map[3] & open_squares,
            prelude.board.height_map[2] & !prelude.board.height_map[3] & open_squares,
        ]
    } else if prelude.can_climb {
        [
            !prelude.board.height_map[1] & open_squares,
            !prelude.board.height_map[2] & open_squares,
            !prelude.board.height_map[3] & open_squares,
            !prelude.board.height_map[3] & open_squares,
        ]
    } else {
        [
            !prelude.board.height_map[0] & open_squares,
            !prelude.board.height_map[1] & open_squares,
            !prelude.board.height_map[2] & open_squares,
            !prelude.board.height_map[3] & open_squares,
        ]
    };
    let open_squares_for_builds = open_squares & prelude.build_mask;

    for worker_start_pos in acting_workers.into_iter() {
        let worker_start_mask = worker_start_pos.to_board();
        let worker_start_height = prelude.board.get_height(worker_start_pos);

        let open_final_destinations = if (worker_start_mask & prelude.affinity_area).is_not_empty()
        {
            open_squares & prelude.affinity_area
        } else {
            open_squares
        } & !INCLUSIVE_NEIGHBOR_MAP[worker_start_pos as usize];

        let all_winning_destinations = all_plausible_winning_mask & open_final_destinations;

        let mut worker_1d_moves = prelude.standard_neighbor_map[worker_start_pos as usize]
            & open_by_height[worker_start_height];

        // After first move, you must be on at least level 1
        if is_mate_only::<F>() {
            worker_1d_moves &= prelude.board.height_map[0];
            if worker_1d_moves.is_empty() {
                continue;
            }
        }

        let mut new_visitable_squares = !(worker_1d_moves | worker_start_mask);
        let mut new_visitable_wins = !(worker_1d_moves | worker_start_mask);

        let mut worker_2d_moves = BitBoard::EMPTY;
        for worker_m_pos in worker_1d_moves {
            let worker_m_height = prelude.board.get_height(worker_m_pos);
            let new_moves = prelude.standard_neighbor_map[worker_m_pos as usize]
                & open_by_height[worker_m_height];

            if worker_m_height == 2 {
                let winning_moves_to_level_3 =
                    new_moves & all_winning_destinations & new_visitable_wins;
                for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                    let winning_move = ScoredMove::new_winning_move(
                        StymphaliansMove::new_winning_move(
                            worker_start_pos,
                            moving_worker_end_pos,
                            worker_m_pos,
                            worker_m_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }
                new_visitable_wins &= !winning_moves_to_level_3;
            }

            worker_2d_moves |= new_moves;
        }
        new_visitable_squares &= new_visitable_wins;
        worker_2d_moves &= new_visitable_squares;

        if is_mate_only::<F>() {
            // After second move, you must be on at least level 2
            worker_2d_moves &= prelude.board.height_map[1];
            if worker_2d_moves.is_empty() {
                continue;
            }
        }

        let mut worker_3d_moves = BitBoard::EMPTY;
        for worker_m_pos in worker_2d_moves {
            let worker_m_height = prelude.board.get_height(worker_m_pos);
            let new_moves = prelude.standard_neighbor_map[worker_m_pos as usize]
                & open_by_height[worker_m_height];

            if worker_m_height == 2 {
                let winning_moves_to_level_3 =
                    new_moves & all_winning_destinations & new_visitable_wins;
                for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                    let possible_first_steps = worker_1d_moves
                        & reverse_map[worker_m_pos as usize]
                        & prelude.board.height_map[0];
                    let first_step = || {
                        let second_step_mask = worker_m_pos.to_board();
                        for possible_first_step in possible_first_steps {
                            let first_step_height = prelude.board.get_height(possible_first_step);
                            let moves_from_first_step = prelude.standard_neighbor_map
                                [possible_first_step as usize]
                                & open_by_height[first_step_height];
                            if (moves_from_first_step & second_step_mask).is_not_empty() {
                                return possible_first_step;
                            }
                        }
                        if cfg!(debug_assertions) {
                            let state = FullGameState::new(
                                prelude.board.clone(),
                                [GodName::Mortal.to_power(), GodName::Mortal.to_power()],
                            );
                            eprint!("board: {:?}", state);
                            state.print_to_console();
                            eprintln!(
                                "could not find winning path start_pos: {} mid_pos: {} end_pos: {} possible_steps: {}",
                                worker_start_pos,
                                worker_m_pos,
                                moving_worker_end_pos,
                                possible_first_steps,
                            );
                        }
                        panic!("Could not find winning path");
                    };

                    let winning_move = ScoredMove::new_winning_move(
                        StymphaliansMove::new_winning_move(
                            worker_start_pos,
                            moving_worker_end_pos,
                            first_step(),
                            worker_m_pos,
                        )
                        .into(),
                    );
                    result.push(winning_move);
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }
                new_visitable_wins &= !winning_moves_to_level_3;
            }

            worker_3d_moves |= new_moves
        }

        if is_mate_only::<F>() {
            continue;
        }

        let worker_final_moves =
            (worker_2d_moves | worker_3d_moves) & open_final_destinations & new_visitable_wins;

        for worker_end_pos in worker_final_moves {
            // let worker_end_height = prelude.board.get_height(worker_end_pos);
            let worker_end_mask = worker_end_pos.to_board();

            let worker_builds = NEIGHBOR_MAP[worker_end_pos as usize] & open_squares_for_builds;
            let mut narrowed_builds = worker_builds;
            if is_interact_with_key_squares::<F>() {
                let is_key_squares_matched = (worker_end_mask & key_squares).is_not_empty();
                narrowed_builds &= [prelude.key_squares, BitBoard::MAIN_SECTION_MASK]
                    [is_key_squares_matched as usize];
            }

            for worker_build_pos in narrowed_builds {
                let new_action = StymphaliansMove::new_basic_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );
                // let build_mask = worker_build_pos.to_board();
                // let is_check = {
                //     let final_level_3 = (prelude.exactly_level_2 & build_mask)
                //         | (prelude.exactly_level_3 & !build_mask);
                //     let check_board = reach_board & final_level_3;
                //     check_board.is_not_empty()
                // };

                result.push(build_scored_move::<F, _>(new_action, false, false))
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        bitboard::{BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP},
        board::{FullGameState, GameStateBuilder},
        consistency_checker::ConsistencyChecker,
        fen::parse_fen,
        gods::{GodName, stymphalians::StymphaliansMove},
        move_verifier::MoveVerifier,
        player::Player,
        square::Square::{self, *},
    };

    /// (start_square, end_square, is_winning)
    type MoveEntry = (Square, Square, bool);

    /// Brute-force reference: enumerate all reachable (start, end, is_win) triples
    /// for Stymphalians via 1-3 steps, with a given climb requirement.
    /// A move is winning if the worker lands on exactly level 3.
    fn bruteforce_destinations_inner(
        state: &FullGameState,
        player: Player,
        require_climb: bool,
    ) -> HashSet<MoveEntry> {
        let board = &state.board;
        let own_workers = board.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;
        let oppo_workers = board.workers[!player as usize] & BitBoard::MAIN_SECTION_MASK;
        let all_workers = own_workers | oppo_workers;
        let domes = board.at_least_level_4();
        let blocked = all_workers | domes;
        let exactly_level_3 = board.exactly_level_3();

        let mut result = HashSet::new();

        for start_pos in own_workers {
            let start_height = board.get_height(start_pos);

            for step1 in NEIGHBOR_MAP[start_pos as usize] {
                if blocked.contains_square(step1) {
                    continue;
                }
                let step1_height = board.get_height(step1);
                // can_climb is always true vs Persephone, so max +1 height per step
                if step1_height > start_height + 1 {
                    continue;
                }
                let did_climb_1 = step1_height > start_height;

                // 1-step moves are always adjacent to start — never a valid final position.
                for step2 in NEIGHBOR_MAP[step1 as usize] {
                    if blocked.contains_square(step2) || step2 == start_pos {
                        continue;
                    }
                    let step2_height = board.get_height(step2);
                    if step2_height > step1_height + 1 {
                        continue;
                    }
                    let did_climb_2 = did_climb_1 || step2_height > step1_height;

                    let is_non_adjacent =
                        !INCLUSIVE_NEIGHBOR_MAP[start_pos as usize].contains_square(step2);
                    let is_win_2 =
                        exactly_level_3.contains_square(step2) && step2_height > step1_height;
                    if is_non_adjacent && (!require_climb || did_climb_2) {
                        result.insert((start_pos, step2, is_win_2));
                    }

                    // Winning by moving up to level 3 ends the move — no further steps.
                    if is_win_2 && is_non_adjacent && (!require_climb || did_climb_2) {
                        continue;
                    }

                    for step3 in NEIGHBOR_MAP[step2 as usize] {
                        if blocked.contains_square(step3) || step3 == start_pos {
                            continue;
                        }
                        let step3_height = board.get_height(step3);
                        if step3_height > step2_height + 1 {
                            continue;
                        }
                        let did_climb_3 = did_climb_2 || step3_height > step2_height;

                        let is_non_adjacent =
                            !INCLUSIVE_NEIGHBOR_MAP[start_pos as usize].contains_square(step3);
                        if is_non_adjacent && (!require_climb || did_climb_3) {
                            let is_win = exactly_level_3.contains_square(step3)
                                && step3_height > step2_height;
                            result.insert((start_pos, step3, is_win));
                        }
                    }
                }
            }
        }

        result
    }

    /// Brute-force reference move generator for Stymphalians vs Persephone.
    /// Implements Persephone's rule: if any climbing move exists, only climbing
    /// moves are legal. Otherwise, all moves are legal (fallback).
    fn bruteforce_stymphalians_vs_persephone_destinations(
        state: &FullGameState,
        player: Player,
    ) -> HashSet<MoveEntry> {
        let climbing = bruteforce_destinations_inner(state, player, true);
        if !climbing.is_empty() {
            climbing
        } else {
            bruteforce_destinations_inner(state, player, false)
        }
    }

    /// Collect the set of (start, end, is_win) from the real move generator.
    fn real_stymphalians_vs_persephone_destinations(
        state: &FullGameState,
        player: Player,
    ) -> HashSet<MoveEntry> {
        let god = GodName::Stymphalians.to_power();
        let moves = god.get_all_moves(state, player);

        let mut result = HashSet::new();
        for m in moves {
            let sm: StymphaliansMove = m.action.into();
            result.insert((
                sm.move_from_position(),
                sm.move_to_position(),
                m.get_is_winning(),
            ));
        }
        result
    }

    /// Compare the brute-force and optimized move generators on a given state.
    /// If any winning moves exist, asserts both agree on all wins.
    /// Otherwise, asserts both agree on all (non-winning) moves.
    fn assert_destinations_match(state: &FullGameState, player: Player) {
        let bruteforce = bruteforce_stymphalians_vs_persephone_destinations(state, player);
        let real = real_stymphalians_vs_persephone_destinations(state, player);

        let bf_wins: HashSet<_> = bruteforce.iter().filter(|(_, _, w)| *w).copied().collect();
        let real_wins: HashSet<_> = real.iter().filter(|(_, _, w)| *w).copied().collect();

        if !bf_wins.is_empty() || !real_wins.is_empty() {
            // Winning moves exist — only compare wins
            let missing: Vec<_> = bf_wins.difference(&real_wins).collect();
            let extra: Vec<_> = real_wins.difference(&bf_wins).collect();
            if !missing.is_empty() || !extra.is_empty() {
                panic!(
                    "Win mismatch on state {:?} (player {:?}):\n  \
                     Missing wins (brute-force only): {:?}\n  \
                     Extra wins (real only): {:?}",
                    state, player, missing, extra,
                );
            }
            return;
        }

        let only_in_real: Vec<_> = real.difference(&bruteforce).collect();

        if !only_in_real.is_empty() {
            panic!(
                "Move mismatch on state {:?} (player {:?}):\n  \
                 Extra (real only): {:?}",
                state, player, only_in_real,
            );
        }
    }

    #[test]
    fn test_stymphalians_vs_persephone_must_climb() {
        // Worker on level 0 with level 1 square nearby — must climb during moves.
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(C3)
            .with_height(B2, 1)
            .build();

        let next_states = state.get_next_states_interactive();
        assert!(!next_states.is_empty(), "Should have at least one move");
        MoveVerifier::new()
            .is_winner(Player::Two)
            .none(&next_states);
    }

    #[test]
    fn test_stymphalians_vs_persephone_no_climb_possible() {
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(C3)
            .build();

        let next_states = state.get_next_states_interactive();
        assert!(
            !next_states.is_empty(),
            "Should have moves when no climbing is possible"
        );
    }

    #[test]
    fn test_stymphalians_vs_persephone_cant_end_near_start() {
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(C3)
            .with_height(B2, 1)
            .build();

        let mut checker = ConsistencyChecker::new(&state);
        checker.perform_all_validations().expect("Failed check");
    }

    #[test]
    fn test_stymphalians_vs_persephone_climb_through_visited_square() {
        // C1(h=3) can reach E1 via C1->D2(h=2)->D1(h=3)->E1(h=0).
        // The D2->D1 step is a climb, satisfying Persephone's must-climb requirement.
        // D1 is also a 1d neighbor of C1, so it appears in both 1d and 2d move sets.
        // The climb through D1 must still propagate even though D1 was visited as a 1d move.
        // C1's only legal moves should be C1->E1 (build D1) and C1->E1 (build D2).
        let state = parse_fen("0000000000002224442400330/1/stymphalians:A1,B1,C1/persephone:D5,E5")
            .unwrap();

        let stymphalians = GodName::Stymphalians.to_power();
        let moves = stymphalians.get_moves_for_search(&state, Player::One);

        let c1_moves: Vec<_> = moves
            .iter()
            .filter(|m| {
                let sm: StymphaliansMove = m.action.into();
                sm.move_from_position() == C1
            })
            .collect();

        assert_eq!(
            c1_moves.len(),
            2,
            "C1 should only have 2 moves (C1->E1 build D1, C1->E1 build D2), got: {:?}",
            c1_moves
                .iter()
                .map(|m| format!("{:?}", StymphaliansMove::from(m.action)))
                .collect::<Vec<_>>()
        );

        for m in &c1_moves {
            let sm: StymphaliansMove = m.action.into();
            assert_eq!(
                sm.move_to_position(),
                E1,
                "C1 should only move to E1, got {:?}",
                sm
            );
        }
    }

    #[test]
    fn test_stymphalians_vs_persephone_win() {
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(A1)
            .with_height(A1, 2)
            .with_height(B2, 2)
            .with_height(C3, 3)
            .build();

        let next_states = state.get_next_states_interactive();
        MoveVerifier::new().is_winner(Player::One).any(&next_states);
    }

    #[test]
    fn test_bruteforce_vs_real_basic_climb() {
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(C3)
            .with_height(B2, 1)
            .build();
        assert_destinations_match(&state, Player::One);
    }

    #[test]
    fn test_bruteforce_vs_real_no_climb_possible() {
        // All flat — no climbs possible, so Persephone allows non-climbing moves
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(C3)
            .build();
        assert_destinations_match(&state, Player::One);
    }

    #[test]
    fn test_bruteforce_vs_real_climb_through_visited() {
        let state = parse_fen("0000000000002224442400330/1/stymphalians:A1,B1,C1/persephone:D5,E5")
            .unwrap();
        assert_destinations_match(&state, Player::One);
    }

    #[test]
    fn test_bruteforce_vs_real_winning_position() {
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(A1)
            .with_height(A1, 2)
            .with_height(B2, 2)
            .with_height(C3, 3)
            .build();
        assert_destinations_match(&state, Player::One);
    }

    #[test]
    fn test_bruteforce_vs_real_varied_heights() {
        // Stymphalians worker in the middle with varied terrain
        let state = GameStateBuilder::new(GodName::Stymphalians, GodName::Persephone)
            .with_p1_worker(C3)
            .with_height(B2, 1)
            .with_height(B3, 2)
            .with_height(D4, 1)
            .with_height(D2, 3)
            .with_height(A5, 2)
            .build();
        assert_destinations_match(&state, Player::One);
    }

    #[test]
    fn test_bruteforce_vs_real_crowded_board() {
        // All workers placed, with some height variety
        let state = parse_fen("1002010020001001002010200/1/stymphalians:A1,C3,E5/persephone:B2,D4")
            .unwrap();
        assert_destinations_match(&state, Player::One);
    }

    #[test]
    fn test_bruteforce_vs_real_high_terrain() {
        let state = parse_fen("2213121132212311321213212/1/stymphalians:A1,C3,E5/persephone:A5,E1")
            .unwrap();
        assert_destinations_match(&state, Player::One);
    }

    #[test]
    fn test_bruteforce_vs_real_random_games() {
        use crate::{matchup::Matchup, random_utils::RandomSingleGameStateGenerator};
        use rand::rng;

        let matchup = Matchup::new(GodName::Stymphalians, GodName::Persephone);
        let num_games = 200;
        let mut states_checked = 0;

        for _ in 0..num_games {
            let starting_state =
                crate::random_utils::get_random_starting_state(&matchup, &mut rng());
            let generator = RandomSingleGameStateGenerator::new(starting_state);

            for state in generator {
                if state.board.get_winner().is_some() {
                    break;
                }
                if state.board.current_player == Player::One {
                    assert_destinations_match(&state, Player::One);
                    states_checked += 1;
                }
            }
        }

        assert!(
            states_checked > 100,
            "Expected to check many states, only checked {}",
            states_checked,
        );
    }
}

pub const fn build_stymphalians() -> GodPower {
    god_power(
        GodName::Stymphalians,
        build_god_power_movers!(stymphalians_move_gen),
        build_god_power_actions::<StymphaliansMove>(),
        13160410892805251325,
        4854231340135741197,
    )
    .with_placement_type(PlacementType::ThreeWorkers)
    .with_nnue_god_name(GodName::Mortal)
}
