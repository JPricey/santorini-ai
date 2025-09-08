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
        harpies::slide_position,
        move_helpers::{
            build_scored_move, get_basic_moves_from_raw_data_with_custom_blockers,
            get_generator_prelude_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

pub const APOLLO_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const APOLLO_MOVE_TO_POSITION_OFFSET: usize = APOLLO_MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_BUILD_POSITION_OFFSET: usize = APOLLO_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_SWAP_MOVE_OFFSET: usize = APOLLO_BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ApolloMove(pub MoveData);

impl Into<GenericMove> for ApolloMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ApolloMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ApolloMove {
    pub fn new_apollo_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        swap_from_square: Option<Square>,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << APOLLO_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << APOLLO_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << APOLLO_BUILD_POSITION_OFFSET)
            | ((swap_from_square.map_or(25 as MoveData, |s| s as MoveData))
                << APOLLO_SWAP_MOVE_OFFSET);

        Self(data)
    }

    pub fn new_apollo_winning_move(
        move_from_position: Square,
        move_to_position: Square,
        swap_from_square: Option<Square>,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << APOLLO_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << APOLLO_MOVE_TO_POSITION_OFFSET)
            | ((swap_from_square.map_or(25 as MoveData, |s| s as MoveData))
                << APOLLO_SWAP_MOVE_OFFSET)
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
        Square::from((self.0 >> APOLLO_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn swap_from_square(self) -> Option<Square> {
        let value = (self.0 >> APOLLO_SWAP_MOVE_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ApolloMove {
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
        } else if self.swap_from_square().is_some() {
            write!(f, "{}<>{}^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for ApolloMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![PartialAction::SelectWorker(self.move_from_position())];

        if let Some(swap_square) = self.swap_from_square() {
            res.push(PartialAction::MoveWorkerWithSwap(
                self.move_to_position(),
                swap_square,
            ));
        } else {
            res.push(PartialAction::MoveWorker(self.move_to_position()));
        }

        if !self.get_is_winning() {
            res.push(PartialAction::Build(self.build_position()));
        }

        return vec![res];
    }

    fn make_move(self, board: &mut BoardState) {
        let from_mask = BitBoard::as_mask(self.move_from_position());
        let to_mask = BitBoard::as_mask(self.move_to_position());
        board.worker_xor(board.current_player, from_mask | to_mask);

        if let Some(swap_from_square) = self.swap_from_square() {
            board.worker_xor(
                !board.current_player,
                from_mask | BitBoard::as_mask(swap_from_square),
            );
        }

        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);
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
        res = res * 2 + self.swap_from_square().is_some() as usize;

        res
    }
}

pub(super) fn apollo_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(apollo_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut worker_moves = get_basic_moves_from_raw_data_with_custom_blockers::<MUST_CLIMB>(
            &prelude,
            worker_start_state.worker_start_pos,
            worker_start_state.worker_start_mask,
            worker_start_state.worker_start_height,
            worker_start_state.other_own_workers,
        );

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;

            for worker_end_pos in moves_to_level_3.into_iter() {
                let swap_square =
                    if (BitBoard::as_mask(worker_end_pos) & prelude.oppo_workers).is_empty() {
                        None
                    } else {
                        Some(worker_end_pos)
                    };
                let winning_move = ScoredMove::new_winning_move(
                    ApolloMove::new_apollo_winning_move(
                        worker_start_pos,
                        worker_end_pos,
                        swap_square,
                    )
                    .into(),
                );
                result.push(winning_move);
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }

            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);

        for mut worker_end_pos in worker_moves {
            let mut worker_end_mask = BitBoard::as_mask(worker_end_pos);

            let is_swap = (BitBoard::as_mask(worker_end_pos) & prelude.oppo_workers).is_not_empty();
            let mut final_other_workers = prelude.oppo_workers;
            let mut final_build_mask = prelude.build_mask;
            let mut swap_square = None;

            let mut swap_mask = BitBoard::EMPTY;
            if is_swap {
                final_other_workers ^= worker_end_mask | worker_start_state.worker_start_mask;
                final_build_mask =
                    prelude.other_god.get_build_mask(final_other_workers) | prelude.exactly_level_3;
                swap_square = Some(worker_end_pos);
                swap_mask = BitBoard::as_mask(worker_end_pos);
            }

            if prelude.is_against_harpies {
                worker_end_pos = slide_position(&prelude.board, worker_start_pos, worker_end_pos);
                worker_end_mask = BitBoard::as_mask(worker_end_pos);
            }

            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_state.worker_start_height;
            let is_now_lvl_2 = (worker_end_height == 2) as u32;

            let self_blockers =
                prelude.domes | worker_start_state.other_own_workers | worker_end_mask;
            let unblocked_squares_for_builds = !(self_blockers | final_other_workers);
            let unblocked_squares_for_checks = !self_blockers;

            let end_neighbors = NEIGHBOR_MAP[worker_end_pos as usize];
            let mut worker_builds = end_neighbors & unblocked_squares_for_builds & final_build_mask;

            if is_interact_with_key_squares::<F>() {
                if ((worker_start_state.worker_start_mask
                    & BitBoard::CONDITIONAL_MASK[is_swap as usize]
                    | worker_end_mask
                    | swap_mask)
                    & key_squares)
                    .is_empty()
                {
                    worker_builds &= key_squares;
                }
            }

            let reach_board = if prelude.is_against_hypnus
                && (other_threatening_workers.count_ones() + is_now_lvl_2) < 2
            {
                BitBoard::EMPTY
            } else {
                (other_threatening_neighbors
                    | (end_neighbors & BitBoard::CONDITIONAL_MASK[is_now_lvl_2 as usize]))
                    & unblocked_squares_for_checks
                    & prelude.win_mask
            };

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = ApolloMove::new_apollo_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                    swap_square,
                );
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    is_improving,
                ))
            }
        }
    }

    result
}

pub const fn build_apollo() -> GodPower {
    god_power(
        GodName::Apollo,
        build_god_power_movers!(apollo_move_gen),
        build_god_power_actions::<ApolloMove>(),
        3394957705078584374,
        7355591628209476781,
    )
}
