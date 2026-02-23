use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, PUSH_MAPPING},
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
            get_standard_reach_board_from_parts, get_worker_end_move_state,
            get_worker_next_build_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const CHARON_MOVE_FROM_POSITION_OFFSET: usize = 0;
const CHARON_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const CHARON_BUILD_POSITION_OFFSET: usize = CHARON_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const CHARON_FLIP_FROM_POSITION_OFFSET: usize = CHARON_BUILD_POSITION_OFFSET + POSITION_WIDTH;
const CHARON_FLIP_TO_POSITION_OFFSET: usize = CHARON_FLIP_FROM_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct CharonV2Move(pub MoveData);

impl Into<GenericMove> for CharonV2Move {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for CharonV2Move {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl CharonV2Move {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << CHARON_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << CHARON_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << CHARON_BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << CHARON_FLIP_FROM_POSITION_OFFSET);

        Self(data)
    }

    fn new_flip_move(
        move_from_position: Square,
        build_position: Square,
        flip_from_position: Square,
        flip_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << CHARON_MOVE_FROM_POSITION_OFFSET)
            | ((25 as MoveData) << CHARON_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << CHARON_BUILD_POSITION_OFFSET)
            | ((flip_from_position as MoveData) << CHARON_FLIP_FROM_POSITION_OFFSET)
            | ((flip_to_position as MoveData) << CHARON_FLIP_TO_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << CHARON_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << CHARON_MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    fn maybe_move_to_position(&self) -> Option<Square> {
        let value = (self.0 >> CHARON_MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> CHARON_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn flip_from_position(&self) -> Square {
        let value = (self.0 >> CHARON_FLIP_FROM_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        Square::from(value)
    }

    fn flip_to_position(&self) -> Square {
        Square::from((self.0 >> CHARON_FLIP_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for CharonV2Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        if let Some(move_to) = self.maybe_move_to_position() {
            if self.get_is_winning() {
                write!(f, "{}>{}#", move_from, move_to)
            } else {
                write!(f, "{}>{}^{}", move_from, move_to, self.build_position())
            }
        } else {
            write!(
                f,
                "{}({}>{})^{}",
                move_from,
                self.flip_from_position(),
                self.flip_to_position(),
                self.build_position()
            )
        }
    }
}

impl GodMove for CharonV2Move {
    fn move_to_actions(self, _board: &BoardState, _player: Player, _other_god: StaticGod) -> Vec<FullAction> {
        let mut result = vec![];

        result.push(PartialAction::SelectWorker(self.move_from_position()));
        if let Some(move_to) = self.maybe_move_to_position() {
            result.push(PartialAction::MoveWorker(move_to.into()));
        } else {
            result.push(PartialAction::ForceOpponentWorker(
                self.flip_from_position(),
                self.flip_to_position(),
            ));
        }

        if !self.get_is_winning() {
            result.push(PartialAction::Build(self.build_position()));
        }

        return vec![result];
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        if let Some(move_to) = self.maybe_move_to_position() {
            let xor = self.move_from_position().to_board() ^ move_to.to_board();
            board.worker_xor(player, xor);

            if self.get_is_winning() {
                board.set_winner(player);
                return;
            }
        } else {
            board.oppo_worker_xor(
                other_god,
                !player,
                self.flip_from_position().to_board() ^ self.flip_to_position().to_board(),
            );
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        if let Some(move_to) = self.maybe_move_to_position() {
            self.move_from_position().to_board() | move_to.to_board()
        } else {
            BitBoard::EMPTY
        }
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        if let Some(move_to) = self.maybe_move_to_position() {
            helper.add_square_with_height(board, move_to);
            helper.add_square_with_height(board, self.build_position());
        } else {
            helper.add_square_with_height(board, self.flip_from_position());
            helper.add_only_square_height(board, self.flip_to_position());
            helper.add_square_with_height(board, self.build_position());
        }
        helper.get()
    }
}

fn charon_v2_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(charon_v2_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let flippable_oppo_workers = state.board.workers[!player as usize] & !prelude.domes_and_frozen;
    let all_starting_blocked_squares =
        prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, CharonV2Move, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                CharonV2Move::new_winning_move,
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
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = CharonV2Move::new_basic_move(
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

        if MUST_CLIMB {
            continue;
        }

        // Flips
        let possible_flips = NEIGHBOR_MAP[worker_start_pos as usize] & flippable_oppo_workers;
        for flip_start_pos in possible_flips {
            let Some(flip_dest) = PUSH_MAPPING[flip_start_pos as usize][worker_start_pos as usize]
            else {
                continue;
            };
            let flip_start_mask = BitBoard::as_mask(flip_start_pos);
            let flip_dest_mask = BitBoard::as_mask(flip_dest);
            if (flip_dest_mask & all_starting_blocked_squares).is_not_empty() {
                continue;
            }
            let new_oppo_workers = prelude.oppo_workers ^ flip_start_mask ^ flip_dest_mask;
            let all_blockers_after_flip =
                all_starting_blocked_squares ^ flip_start_mask ^ flip_dest_mask;
            let all_open_after_flip = !all_blockers_after_flip;
            let new_build_mask =
                prelude.other_god.get_build_mask(new_oppo_workers) | prelude.exactly_level_3;

            let all_possible_builds = NEIGHBOR_MAP[worker_start_state.worker_start_pos as usize]
                & all_open_after_flip
                & new_build_mask;

            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let interact_board = key_squares & (flip_start_mask | flip_dest_mask);

                if interact_board.is_empty() {
                    narrowed_builds &= prelude.key_squares;
                }
            }

            let reach_board = get_standard_reach_board_from_parts::<F>(
                &prelude,
                worker_next_moves.other_threatening_workers,
                worker_next_moves.other_threatening_neighbors,
                worker_start_state.worker_start_pos,
                (prelude
                    .board
                    .get_height(worker_start_state.worker_start_pos)
                    == 2) as u32,
                all_open_after_flip,
            );

            for worker_build_pos in narrowed_builds {
                let build_mask = worker_build_pos.to_board();

                let new_action = CharonV2Move::new_flip_move(
                    worker_start_pos,
                    worker_build_pos,
                    flip_start_pos,
                    flip_dest,
                );

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }
    }

    if prelude.is_against_hypnus && !is_mate_only::<F>() {
        // also flip unmoveable workers
        let flip_only_workers = prelude.own_workers ^ prelude.acting_workers;

        for worker_start_pos in flip_only_workers {
            let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

            // Flips
            let possible_flips = NEIGHBOR_MAP[worker_start_pos as usize] & flippable_oppo_workers;
            for flip_start_pos in possible_flips {
                let Some(flip_dest) =
                    PUSH_MAPPING[flip_start_pos as usize][worker_start_pos as usize]
                else {
                    continue;
                };
                let flip_start_mask = BitBoard::as_mask(flip_start_pos);
                let flip_dest_mask = BitBoard::as_mask(flip_dest);
                if (flip_dest_mask & all_starting_blocked_squares).is_not_empty() {
                    continue;
                }
                let all_blockers_after_flip =
                    all_starting_blocked_squares ^ flip_start_mask ^ flip_dest_mask;
                let all_open_after_flip = !all_blockers_after_flip;

                let all_possible_builds = NEIGHBOR_MAP
                    [worker_start_state.worker_start_pos as usize]
                    & all_open_after_flip;

                let mut narrowed_builds = all_possible_builds;
                if is_interact_with_key_squares::<F>() {
                    let interact_board = key_squares & (flip_start_mask | flip_dest_mask);

                    if interact_board.is_empty() {
                        narrowed_builds &= prelude.key_squares;
                    }
                }

                for worker_build_pos in narrowed_builds {
                    let new_action = CharonV2Move::new_flip_move(
                        worker_start_pos,
                        worker_build_pos,
                        flip_start_pos,
                        flip_dest,
                    );

                    result.push(build_scored_move::<F, _>(new_action, false, false));
                }
            }
        }
    }

    result
}

pub(crate) const fn build_charon_v2() -> GodPower {
    god_power(
        GodName::CharonV2,
        build_god_power_movers!(charon_v2_move_gen),
        build_god_power_actions::<CharonV2Move>(),
        2583676188350615135,
        4632020203302486643,
    )
}
