use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
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
            GeneratorPreludeState, build_scored_move, get_basic_moves, get_generator_prelude_state,
            get_worker_end_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const WORKER_SPECIAL_ACTION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct HydraMove(pub MoveData);

impl GodMove for HydraMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        res.push(PartialAction::PlaceWorker(self.special_worker_position()));

        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        if self.get_is_winning() {
            board.worker_xor(
                player,
                self.move_from_position().to_board() ^ self.move_to_position().to_board(),
            );

            board.set_winner(player);
            return;
        }

        board.worker_xor(
            player,
            self.move_from_position().to_board()
                ^ self.move_to_position().to_board()
                ^ self.special_worker_position().to_board(),
        );
        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_square_with_height(board, self.special_worker_position());
        helper.get()
    }
}

impl Into<GenericMove> for HydraMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for HydraMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl HydraMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        special_worker_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((special_worker_position as MoveData) << WORKER_SPECIAL_ACTION_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn special_worker_position(self) -> Square {
        Square::from((self.0 >> WORKER_SPECIAL_ACTION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for HydraMove {
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
            write!(
                f,
                "{}>{}^{} H{}",
                move_from,
                move_to,
                build,
                self.special_worker_position()
            )
        }
    }
}

fn _compute_new_worker_spots(
    prelude: &GeneratorPreludeState,
    worker_end_pos: Square,
    unblocked_squares: BitBoard,
    build_square: BitBoard,
) -> BitBoard {
    let ending_neighbors = NEIGHBOR_MAP[worker_end_pos as usize] & unblocked_squares;

    {
        let valid_0s = ending_neighbors & prelude.exactly_level_0 & !build_square;
        if valid_0s.is_not_empty() {
            return valid_0s;
        }
    }

    {
        let valid_1s = ending_neighbors
            & (prelude.exactly_level_0 & build_square | prelude.exactly_level_1 & !build_square);
        if valid_1s.is_not_empty() {
            return valid_1s;
        }
    }

    {
        let valid_2s = ending_neighbors
            & (prelude.exactly_level_1 & build_square | prelude.exactly_level_2 & !build_square);
        if valid_2s.is_not_empty() {
            return valid_2s;
        }
    }

    {
        let valid_3s = ending_neighbors
            & (prelude.exactly_level_2 & build_square | prelude.exactly_level_3 & !build_square);
        if valid_3s.is_not_empty() {
            return valid_3s;
        }
    }

    BitBoard::EMPTY
}

pub(super) fn hydra_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(hydra_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    // Can't move with single worker vs hypnus
    if prelude.other_god.god_name == GodName::Hypnus && prelude.own_workers.count_ones() < 2 {
        return result;
    }

    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let mut all_worker_neighbors = BitBoard::EMPTY;
    let mut all_worker_double_neighbors = BitBoard::EMPTY;

    if !is_mate_only::<F>() {
        for worker_pos in prelude.own_workers {
            let worker_neighbors = NEIGHBOR_MAP[worker_pos as usize];
            all_worker_double_neighbors |= worker_neighbors & all_worker_neighbors;
            all_worker_neighbors |= worker_neighbors;
        }
    }

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_moves = get_basic_moves::<MUST_CLIMB>(&prelude, &worker_start_state);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, HydraMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                HydraMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let worker_neighbors_without_own_worker = all_worker_neighbors
            ^ NEIGHBOR_MAP[worker_start_pos as usize]
            | all_worker_double_neighbors;

        let threatening_others = worker_start_state.other_own_workers & prelude.exactly_level_2;
        let mut other_worker_reach = BitBoard::EMPTY;
        let mut other_worker_double_reach = BitBoard::EMPTY;

        for other_worker_pos in threatening_others {
            let worker_reach = prelude.standard_neighbor_map[other_worker_pos as usize];
            other_worker_double_reach |= other_worker_reach & worker_reach;
            other_worker_reach |= worker_reach;
        }

        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);
            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;
            let mut narrowed_builds = all_possible_builds;

            let new_worker_neighbor_map = worker_neighbors_without_own_worker
                | NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize];

            let own_workers_after_move =
                worker_start_state.other_own_workers | worker_end_move_state.worker_end_mask;
            let is_making_new_worker =
                (new_worker_neighbor_map & own_workers_after_move).is_empty();

            if is_making_new_worker {
                let after_move_threats;
                let after_move_reach;

                if worker_end_move_state.worker_end_height == 2 {
                    after_move_threats = threatening_others | worker_end_move_state.worker_end_mask;
                    after_move_reach = other_worker_reach
                        | prelude.standard_neighbor_map
                            [worker_end_move_state.worker_end_pos as usize]
                } else {
                    after_move_threats = threatening_others;
                    after_move_reach = other_worker_reach;
                }

                for worker_build_pos in narrowed_builds {
                    let worker_build_mask = worker_build_pos.to_board();

                    let final_level_3 = ((prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask))
                        & prelude.win_mask;
                    let final_level_2 = (prelude.exactly_level_1 & worker_build_mask)
                        | (prelude.exactly_level_2 & !worker_build_mask);

                    let mut addable_spots = _compute_new_worker_spots(
                        &prelude,
                        worker_end_move_state.worker_end_pos,
                        unblocked_squares,
                        worker_build_mask,
                    );

                    if is_interact_with_key_squares::<F>() {
                        let is_already_matched =
                            ((worker_end_move_state.worker_end_mask | worker_build_mask)
                                & prelude.key_squares)
                                .is_not_empty() as usize;
                        addable_spots &=
                            [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
                    }

                    for worker_to_add_pos in addable_spots {
                        let new_action = HydraMove::new_basic_move(
                            worker_start_pos,
                            worker_end_move_state.worker_end_pos,
                            worker_build_pos,
                            worker_to_add_pos,
                        );

                        let worker_to_add_mask = worker_to_add_pos.to_board();
                        let is_final_worker_on_level_2 =
                            (final_level_2 & worker_to_add_mask).is_not_empty();

                        let is_check = {
                            let final_threats;
                            let final_reach;

                            if is_final_worker_on_level_2 {
                                final_threats = after_move_threats | worker_to_add_mask;
                                final_reach = after_move_reach
                                    | prelude.standard_neighbor_map[worker_to_add_pos as usize];
                            } else {
                                final_threats = after_move_threats;
                                final_reach = after_move_reach;
                            }

                            if prelude.is_against_hypnus && final_threats.count_ones() < 2 {
                                false
                            } else {
                                let check_board = final_reach
                                    & final_level_3
                                    & unblocked_squares
                                    & !worker_to_add_mask;

                                check_board.is_not_empty()
                            }
                        };

                        result.push(build_scored_move::<F, _>(new_action, is_check, true))
                    }
                }
            } else {
                let after_move_threats;
                let after_move_reach;
                let after_move_double_reach;

                if worker_end_move_state.worker_end_height == 2 {
                    let end_state_reach = prelude.standard_neighbor_map
                        [worker_end_move_state.worker_end_pos as usize];
                    after_move_threats = threatening_others | worker_end_move_state.worker_end_mask;
                    after_move_double_reach =
                        other_worker_double_reach | other_worker_reach & end_state_reach;
                    after_move_reach = other_worker_reach | end_state_reach;
                } else {
                    after_move_threats = threatening_others;
                    after_move_reach = other_worker_reach;
                    after_move_double_reach = other_worker_double_reach;
                }

                if is_interact_with_key_squares::<F>() {
                    let is_already_matched = (worker_end_move_state.worker_end_mask
                        & prelude.key_squares)
                        .is_not_empty() as usize;
                    narrowed_builds &=
                        [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
                }

                for worker_build_pos in narrowed_builds {
                    let worker_build_mask = worker_build_pos.to_board();

                    let final_level_3 = ((prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos)))
                        & prelude.win_mask;

                    for worker_to_remove_pos in own_workers_after_move {
                        let new_action = HydraMove::new_basic_move(
                            worker_start_pos,
                            worker_end_move_state.worker_end_pos,
                            worker_build_pos,
                            worker_to_remove_pos,
                        );

                        let is_improving = worker_end_move_state.is_improving
                            && worker_to_remove_pos != worker_end_move_state.worker_end_pos;
                        let final_unblocked = unblocked_squares | worker_to_remove_pos.to_board();

                        let is_check = {
                            if prelude.board.height_lookup[worker_to_remove_pos as usize] == 2 {
                                if prelude.is_against_hypnus && after_move_threats.count_ones() < 3
                                {
                                    false
                                } else {
                                    let final_reach = (after_move_reach
                                        ^ prelude.standard_neighbor_map
                                            [worker_to_remove_pos as usize])
                                        | after_move_double_reach;
                                    let check_board = final_reach & final_level_3 & final_unblocked;
                                    check_board.is_not_empty()
                                }
                            } else {
                                if prelude.is_against_hypnus && after_move_threats.count_ones() < 2
                                {
                                    false
                                } else {
                                    let check_board =
                                        after_move_reach & final_level_3 & final_unblocked;
                                    check_board.is_not_empty()
                                }
                            }
                        };

                        result.push(build_scored_move::<F, _>(
                            new_action,
                            is_check,
                            is_improving,
                        ))
                    }
                }
            }
        }
    }

    result
}

pub const fn build_hydra() -> GodPower {
    god_power(
        GodName::Hydra,
        build_god_power_movers!(hydra_move_gen),
        build_god_power_actions::<HydraMove>(),
        2854659210591727588,
        10142526825370404391,
    )
    .with_nnue_god_name(GodName::Mortal)
}
