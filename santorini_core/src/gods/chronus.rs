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
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, is_stop_on_mate, push_winning_moves,
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

const CHRONUS_DOME_COUNT_TO_WIN: u32 = 5;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ChronusMove(pub MoveData);

impl GodMove for ChronusMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        if self.get_is_winning() {
            if self.0 == MOVE_IS_WINNING_MASK | NULL_MOVE_DATA {
                vec![vec![]]
            } else if let Some(build_pos) = self.maybe_build_position() {
                vec![vec![
                    PartialAction::SelectWorker(self.move_from_position()),
                    PartialAction::MoveWorker(self.move_to_position().into()),
                    PartialAction::Build(build_pos),
                ]]
            } else {
                vec![vec![
                    PartialAction::SelectWorker(self.move_from_position()),
                    PartialAction::MoveWorker(self.move_to_position().into()),
                ]]
            }
        } else {
            vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position().into()),
                PartialAction::Build(self.build_position()),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        if self.get_is_winning() {
            board.set_winner(player);
            if self.0 == MOVE_IS_WINNING_MASK | NULL_MOVE_DATA {
                return;
            }
            board.worker_xor(player, self.move_mask());

            if let Some(build_pos) = self.maybe_build_position() {
                board.build_up(build_pos);
            }
        } else {
            board.worker_xor(player, self.move_mask());
            board.build_up(self.build_position());
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        if self.0 == NULL_MOVE_DATA | MOVE_IS_WINNING_MASK {
            BitBoard::EMPTY
        } else if let Some(build_pos) = self.maybe_build_position() {
            self.move_from_position().to_board()
                | self.move_to_position().to_board()
                | build_pos.to_board()
        } else {
            self.move_from_position().to_board() | self.move_to_position().to_board()
        }
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for ChronusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ChronusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ChronusMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((25 as MoveData) << BUILD_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn new_winning_move_with_build(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    pub fn new_winning_null_move() -> Self {
        let data: MoveData = NULL_MOVE_DATA | MOVE_IS_WINNING_MASK;
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

    pub fn maybe_build_position(self) -> Option<Square> {
        let build_val = (self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if build_val == 25 {
            None
        } else {
            Some(Square::from(build_val))
        }
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ChronusMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            if self.0 == (MOVE_IS_WINNING_MASK | NULL_MOVE_DATA) {
                return write!(f, "#");
            } else if let Some(build_pos) = self.maybe_build_position() {
                return write!(f, "{}>{}^{}#", move_from, move_to, build_pos);
            } else {
                write!(f, "{}>{}#", move_from, move_to)
            }
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

pub(super) fn chronus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(chronus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let dome_count = state.board.height_map[3].count_ones();
    if dome_count >= CHRONUS_DOME_COUNT_TO_WIN {
        result.push(build_scored_move::<F, _>(
            ChronusMove::new_winning_null_move(),
            false,
            false,
        ));
        return result;
    }

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, ChronusMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                ChronusMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() && dome_count < CHRONUS_DOME_COUNT_TO_WIN - 1 {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);
            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;
            let mut narrowed_builds = all_possible_builds;
            let winning_builds = if dome_count == CHRONUS_DOME_COUNT_TO_WIN - 1 {
                prelude.exactly_level_3
            } else {
                BitBoard::EMPTY
            };
            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (worker_end_move_state.worker_end_mask
                    & prelude.key_squares)
                    .is_not_empty() as usize;
                narrowed_builds &= [
                    prelude.key_squares | winning_builds,
                    BitBoard::MAIN_SECTION_MASK,
                ][is_already_matched];
            }

            if is_mate_only::<F>() {
                narrowed_builds &= winning_builds;
            }

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            for worker_build_pos in narrowed_builds {
                let build_mask = worker_build_pos.to_board();
                if is_mate_only::<F>()
                    || dome_count == CHRONUS_DOME_COUNT_TO_WIN - 1
                        && (prelude.exactly_level_3 & build_mask).is_not_empty()
                {
                    result.push(build_scored_move::<F, _>(
                        ChronusMove::new_winning_move_with_build(
                            worker_start_pos,
                            worker_end_move_state.worker_end_pos,
                            worker_build_pos,
                        ),
                        false,
                        false,
                    ));

                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                } else {
                    let new_action = ChronusMove::new_basic_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                    );
                    let is_check = {
                        let final_level_3 = (prelude.exactly_level_2 & build_mask)
                            | (prelude.exactly_level_3 & !build_mask);
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
    }

    result
}

pub(crate) const fn build_chronus() -> GodPower {
    god_power(
        GodName::Chronus,
        build_god_power_movers!(chronus_move_gen),
        build_god_power_actions::<ChronusMove>(),
        14553547426435464403,
        3013502386383907053,
    )
    .with_nnue_god_name(GodName::Mortal)
}
