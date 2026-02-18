use crate::{
    bitboard::{BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP}, board::{BoardState, FullGameState}, build_god_power_movers, gods::{
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
            is_interact_with_key_squares, is_mate_only, is_stop_on_mate,
        },
    }, persephone_check_result, placement::PlacementType, player::Player, square::Square
};

// StymphaliansMove is an exact copy of MortalMove, except with a different blocker board calculation to
// account for the longer moves
// from(5)|to(5)|build(5)|win(1)
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

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    fn move_to_position(&self) -> Square {
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

fn stymphalians_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    // if MUST_CLIMB {
    //     return stymphalians_vs_persephone::<F>(state, player, key_squares);
    // }

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
                let new_action =
                    StymphaliansMove::new_basic_move(worker_start_pos, worker_end_pos, worker_build_pos);
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
