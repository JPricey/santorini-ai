use crate::{
    bitboard::{BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK, NEIGHBOR_MAP},
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
            is_interact_with_key_squares, is_mate_only, is_stop_on_mate,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const DOME_BUILD_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct AsteriaMove(pub MoveData);

impl Into<GenericMove> for AsteriaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AsteriaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AsteriaMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << DOME_BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_dome_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        dome_build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((dome_build_position as MoveData) << DOME_BUILD_POSITION_OFFSET);

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

    fn maybe_dome_build_position(self) -> Option<Square> {
        let value = (self.0 >> DOME_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for AsteriaMove {
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
        } else if let Some(dome_build) = self.maybe_dome_build_position() {
            write!(f, "{}>{}^{} X{}", move_from, move_to, build, dome_build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build,)
        }
    }
}

impl GodMove for AsteriaMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];

        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));

        if let Some(dome_build) = self.maybe_dome_build_position() {
            res.push(PartialAction::Dome(dome_build));
        }

        return vec![res];
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
        if let Some(dome_build) = self.maybe_dome_build_position() {
            board.dome_up(dome_build);
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
        helper.add_maybe_square_with_height(board, self.maybe_dome_build_position());
        helper.get()
    }
}

pub(super) fn asteria_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(asteria_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

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
            if push_winning_moves::<F, AsteriaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                AsteriaMove::new_winning_move,
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

            let did_step_down =
                worker_end_move_state.worker_end_height < worker_start_state.worker_start_height;

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;
            let mut narrowed_builds = all_possible_builds;
            let mut must_interact_squares = BitBoard::MAIN_SECTION_MASK;

            if is_interact_with_key_squares::<F>() {
                let is_already_matched =
                    (worker_end_move_state.worker_end_mask & prelude.key_squares).is_not_empty();

                if is_already_matched {
                    // Noop
                } else if did_step_down {
                    must_interact_squares = key_squares;
                } else {
                    narrowed_builds &= prelude.key_squares;
                    must_interact_squares = key_squares;
                }
            }

            for worker_build_pos in narrowed_builds {
                let worker_build_mask = worker_build_pos.to_board();

                let check_board = {
                    let final_level_3 = (prelude.exactly_level_2
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_3 & !worker_build_mask);
                    reach_board & final_level_3
                };

                if !is_interact_with_key_squares::<F>()
                    || (worker_build_mask & must_interact_squares).is_not_empty()
                {
                    let new_action = AsteriaMove::new_basic_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                    );

                    result.push(build_scored_move::<F, _>(
                        new_action,
                        check_board.is_not_empty(),
                        worker_end_move_state.is_improving,
                    ));
                }

                if !did_step_down {
                    continue;
                }

                let mut dome_spots = BitBoard::MAIN_SECTION_MASK
                    & unblocked_squares
                    & !(prelude.exactly_level_3 & worker_build_mask);

                if is_interact_with_key_squares::<F>()
                    & (worker_build_mask & must_interact_squares).is_empty()
                {
                    dome_spots &= key_squares;
                }

                // Prevent duplicate states...
                // This can happen when:
                // 1. You just domed a square with your regular build
                // 2. You're about to dome another level 3 that is in your building range
                // Prevent this by:
                // 1. If you just domed a spot, prevent other domes in lower spots
                if is_stop_on_mate::<F>() {
                    if (worker_build_mask & prelude.exactly_level_3).is_not_empty() {
                        dome_spots &= !(LOWER_SQUARES_EXCLUSIVE_MASK[worker_build_pos as usize]
                            & all_possible_builds
                            & prelude.exactly_level_3)
                    }
                }

                for dome_pos in dome_spots {
                    let dome_mask = dome_pos.to_board();

                    let new_action = AsteriaMove::new_dome_build_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                        dome_pos,
                    );
                    let is_check = (check_board & !dome_mask).is_not_empty();

                    result.push(build_scored_move::<F, _>(new_action, is_check, false));
                }
            }
        }
    }

    result
}

pub const fn build_asteria() -> GodPower {
    god_power(
        GodName::Asteria,
        build_god_power_movers!(asteria_move_gen),
        build_god_power_actions::<AsteriaMove>(),
        13209756228508321548,
        4520869061511324205,
    )
    .with_nnue_god_name(GodName::Atlas)
}
