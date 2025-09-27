use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, WIND_AWARE_NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_basic_moves_from_raw_data_with_custom_blockers,
            get_generator_prelude_state, get_standard_reach_board, get_worker_end_move_state,
            get_worker_next_build_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, is_stop_on_mate,
            modify_prelude_for_checking_workers, push_winning_moves,
            restrict_moves_by_affinity_area,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET_1: usize = 0;
const MOVE_TO_POSITION_OFFSET_1: usize = MOVE_FROM_POSITION_OFFSET_1 + POSITION_WIDTH;

const MOVE_FROM_POSITION_OFFSET_2: usize = MOVE_TO_POSITION_OFFSET_1 + POSITION_WIDTH;
const MOVE_TO_POSITION_OFFSET_2: usize = MOVE_FROM_POSITION_OFFSET_2 + POSITION_WIDTH;

const BUILD_POSITION_1: usize = MOVE_TO_POSITION_OFFSET_2 + POSITION_WIDTH;
const BUILD_POSITION_2: usize = BUILD_POSITION_1 + POSITION_WIDTH;

/*
 * 20 bits for double move
 * 10 bits for double build
 * 15 bits for regular
 * types of moves:
 * - mortal
 * - double build
 * - Double move * order can matter
 *  - one worker needs to move out of the way of the other.
 *  - Extra complicated for harpies, since it's not clear that this is going to happen
 *  Can maybe ignore ordering for now in repr? and the ui just gets a bit jank?
 *
 *  Ok let's try:
 *  Maybe from, to
 *  Maybe from, to
 *  Maybe build
 *  Maybe build
 *
 *  Also need to handle only 1x worker
 *  - I guess we can do move only when this happens???, since 1x worker is still "all"
 */

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CastorMove(pub MoveData);

impl GodMove for CastorMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        if let Some(from1) = self.maybe_move_from_position_1() {
            let to1 = self.move_to_position_1();
            let mut res = vec![
                PartialAction::SelectWorker(from1),
                PartialAction::MoveWorker(to1.into()),
            ];

            if let Some(from2) = self.maybe_move_from_position_2() {
                let to2 = self.move_to_position_2();

                if to1 == from2 {
                    return vec![vec![
                        PartialAction::SelectWorker(from2),
                        PartialAction::MoveWorker(to2.into()),
                        PartialAction::SelectWorker(from1),
                        PartialAction::MoveWorker(to1.into()),
                    ]];
                } else if to2 == from1 {
                    return vec![vec![
                        PartialAction::SelectWorker(from1),
                        PartialAction::MoveWorker(to1.into()),
                        PartialAction::SelectWorker(from2),
                        PartialAction::MoveWorker(to2.into()),
                    ]];
                } else {
                    return vec![
                        vec![
                            PartialAction::SelectWorker(from1),
                            PartialAction::MoveWorker(to1.into()),
                            PartialAction::SelectWorker(from2),
                            PartialAction::MoveWorker(to2.into()),
                        ],
                        vec![
                            PartialAction::SelectWorker(from2),
                            PartialAction::MoveWorker(to2.into()),
                            PartialAction::SelectWorker(from1),
                            PartialAction::MoveWorker(to1.into()),
                        ],
                    ];
                }
            } else if let Some(build) = self.maybe_build_position_1() {
                res.push(PartialAction::Build(build));
                return vec![res];
            } else {
                return vec![res];
            }
        } else {
            // Double build
            let b1 = self.definite_build_position_1();

            if let Some(build2) = self.maybe_build_position_2() {
                return vec![
                    vec![PartialAction::Build(b1), PartialAction::Build(build2)],
                    vec![PartialAction::Build(build2), PartialAction::Build(b1)],
                ];
            } else {
                return vec![vec![PartialAction::Build(b1)]];
            }
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player) {
        if let Some(move_from_1) = self.maybe_move_from_position_1() {
            let mut move_mask =
                BitBoard::as_mask(move_from_1) ^ BitBoard::as_mask(self.move_to_position_1());

            if let Some(from2) = self.maybe_move_from_position_2() {
                move_mask ^=
                    BitBoard::as_mask(from2) ^ BitBoard::as_mask(self.move_to_position_2());
                board.worker_xor(player, move_mask);

                if self.get_is_winning() {
                    board.set_winner(player);
                }
            } else {
                board.worker_xor(player, move_mask);

                if self.get_is_winning() {
                    board.set_winner(player);
                } else if let Some(build) = self.maybe_build_position_1() {
                    board.build_up(build);
                }
            }
        } else {
            board.build_up(self.definite_build_position_1());
            if let Some(build2) = self.maybe_build_position_2() {
                board.build_up(build2);
            }
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        if let Some(mf_2) = self.maybe_move_from_position_2() {
            BitBoard::as_mask(self.definite_move_from_position_1())
                | BitBoard::as_mask(self.move_to_position_1())
                | BitBoard::as_mask(mf_2)
                | BitBoard::as_mask(self.move_to_position_2())
        } else {
            BitBoard::as_mask(self.definite_move_from_position_1())
                | BitBoard::as_mask(self.move_to_position_1())
        }
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        if let Some(move1) = self.maybe_move_from_position_1() {
            helper.add_square_with_height(board, move1);
            helper.add_square_with_height(board, self.move_to_position_1());

            if let Some(move2) = self.maybe_move_from_position_2() {
                helper.add_square_with_height(board, move2);
                helper.add_square_with_height(board, self.move_to_position_2());
            } else if let Some(build) = self.maybe_build_position_1() {
                helper.add_square_with_height(board, build);
            }
        } else {
            helper.add_value(1, 2);
            helper.add_square_with_height(board, self.definite_build_position_1());

            if let Some(build2) = self.maybe_build_position_2() {
                helper.add_square_with_height(board, build2);
            }
        }

        helper.get()
    }
}

impl Into<GenericMove> for CastorMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for CastorMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl CastorMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((build_position as MoveData) << BUILD_POSITION_1);

        Self(data)
    }

    pub fn new_winning_single_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((25 as MoveData) << BUILD_POSITION_1)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn new_double_move(from1: Square, to1: Square, from2: Square, to2: Square) -> Self {
        let data: MoveData = ((from1 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((to1 as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((from2 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((to2 as MoveData) << MOVE_TO_POSITION_OFFSET_2)
            | ((25 as MoveData) << BUILD_POSITION_1);

        Self(data)
    }

    pub fn new_winning_double_move(from1: Square, to1: Square, from2: Square, to2: Square) -> Self {
        let data: MoveData = ((from1 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((to1 as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((from2 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((to2 as MoveData) << MOVE_TO_POSITION_OFFSET_2)
            | ((25 as MoveData) << BUILD_POSITION_1)
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    pub fn new_double_build(build_1: Square, build_2: Square) -> Self {
        let data: MoveData = ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((build_1 as MoveData) << BUILD_POSITION_1)
            | ((build_2 as MoveData) << BUILD_POSITION_2);
        Self(data)
    }

    pub fn new_single_build(build_1: Square) -> Self {
        let data: MoveData = ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((build_1 as MoveData) << BUILD_POSITION_1)
            | ((25 as MoveData) << BUILD_POSITION_2);
        Self(data)
    }

    pub fn maybe_move_from_position_1(&self) -> Option<Square> {
        let value = (self.0 >> MOVE_FROM_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn definite_move_from_position_1(&self) -> Square {
        let value = (self.0 >> MOVE_FROM_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK;
        Square::from(value)
    }

    // Only call when we know we're doing this kind of move
    pub fn move_to_position_1(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_move_from_position_2(&self) -> Option<Square> {
        let value = (self.0 >> MOVE_FROM_POSITION_OFFSET_2) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    // Only call when we know we're doing this kind of move
    pub fn move_to_position_2(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET_2) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_build_position_1(&self) -> Option<Square> {
        let value = (self.0 >> BUILD_POSITION_1) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn definite_build_position_1(&self) -> Square {
        let value = (self.0 >> BUILD_POSITION_1) as u8 & LOWER_POSITION_MASK;
        Square::from(value)
    }

    pub fn maybe_build_position_2(&self) -> Option<Square> {
        let value = (self.0 >> BUILD_POSITION_2) as u8 & LOWER_POSITION_MASK;
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

impl std::fmt::Debug for CastorMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        if let Some(move1) = self.maybe_move_from_position_1() {
            let mut res = format!("{}>{}", move1, self.move_to_position_1());

            if let Some(move2) = self.maybe_move_from_position_2() {
                res += &format!(" {}>{}", move2, self.move_to_position_2());
            } else if let Some(build) = self.maybe_build_position_1() {
                res += &format!(" ^{}", build);
            }

            if self.get_is_winning() {
                res += "#";
            }

            write!(f, "{}", res)
        } else {
            if let Some(build2) = self.maybe_build_position_2() {
                write!(f, "^{} ^{}", self.definite_build_position_1(), build2)
            } else {
                write!(f, "^{}", self.definite_build_position_1())
            }
        }
    }
}

pub(super) fn castor_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(castor_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let mut did_win = false;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, CastorMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                CastorMove::new_winning_single_move,
            ) {
                did_win = true;
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
                let new_action = CastorMove::new_basic_move(
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

    // Double moves
    {
        let mut own_workers = prelude.own_workers.into_iter();
        let Some(worker_start_1) = own_workers.next() else {
            return result;
        };

        if prelude.is_against_hypnus
            && (prelude.own_workers & prelude.exactly_level_2) != prelude.own_workers
        {
            // noop
        } else if did_win {
            // noop
        } else {
            let non_own_worker_blockers = prelude.domes_and_frozen | prelude.oppo_workers;

            // TODO: handle persephone - should only need to climb with ONE worker
            let start_height_1 = prelude.board.get_height(worker_start_1);
            let start_mask_1 = BitBoard::as_mask(worker_start_1);
            let mut moves_1 = get_basic_moves_from_raw_data_with_custom_blockers::<MUST_CLIMB>(
                &prelude,
                worker_start_1,
                start_mask_1,
                start_height_1,
                non_own_worker_blockers,
            );

            let wins_1 = if start_height_1 == 2 {
                moves_1 & prelude.exactly_level_3 & prelude.win_mask
            } else {
                BitBoard::EMPTY
            };
            moves_1 ^= wins_1;

            if let Some(worker_start_2) = own_workers.next() {
                let start_height_2 = prelude.board.get_height(worker_start_2);
                let start_mask_2 = BitBoard::as_mask(worker_start_2);

                let mut moves_2 = get_basic_moves_from_raw_data_with_custom_blockers::<MUST_CLIMB>(
                    &prelude,
                    worker_start_2,
                    start_mask_2,
                    start_height_2,
                    non_own_worker_blockers,
                );

                let wins_2 = if start_height_2 == 2 {
                    moves_2 & prelude.exactly_level_3 & prelude.win_mask
                } else {
                    BitBoard::EMPTY
                };
                moves_2 ^= wins_2;

                if (wins_2 & start_mask_1).is_not_empty() {
                    for to1 in moves_1 & !start_mask_2 {
                        let new_action = CastorMove::new_winning_double_move(
                            worker_start_1,
                            to1,
                            worker_start_2,
                            worker_start_1,
                        );
                        result.push(ScoredMove::new_winning_move(new_action.into()));
                        if is_stop_on_mate::<F>() {
                            return result;
                        }
                    }
                }

                if (wins_1 & start_mask_2).is_not_empty() {
                    for to2 in moves_2 & !start_mask_1 {
                        let new_action = CastorMove::new_winning_double_move(
                            worker_start_2,
                            to2,
                            worker_start_1,
                            worker_start_2,
                        );
                        result.push(ScoredMove::new_winning_move(new_action.into()));
                        if is_stop_on_mate::<F>() {
                            return result;
                        }
                    }
                }

                if is_mate_only::<F>() {
                    // NOOP - can't mate here anymore
                } else {
                    for to1 in moves_1 {
                        let end_mask_1 = BitBoard::as_mask(to1);
                        let end_height_1 = prelude.board.get_height(to1);
                        let reach1 = if end_height_1 == 2 {
                            WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx][to1 as usize]
                        } else {
                            BitBoard::EMPTY
                        };

                        let mut final_moves_2 = moves_2 & !end_mask_1;
                        if to1 == worker_start_2 {
                            final_moves_2 &= !start_mask_1;
                        }

                        if is_interact_with_key_squares::<F>()
                            && (key_squares & end_mask_1).is_empty()
                        {
                            final_moves_2 &= key_squares;
                        }

                        for to2 in final_moves_2 {
                            let end_height_2 = prelude.board.get_height(to2);
                            let end_mask_2 = BitBoard::as_mask(to2);
                            let end_masks = end_mask_1 | end_mask_2;

                            let new_action = CastorMove::new_double_move(
                                worker_start_1,
                                to1,
                                worker_start_2,
                                to2,
                            );

                            let reach2 = if end_height_2 == 2 {
                                WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx][to2 as usize]
                            } else {
                                BitBoard::EMPTY
                            };

                            let is_check = {
                                if prelude.is_against_hypnus
                                    && (end_height_1 != 2 || end_height_2 != 2)
                                {
                                    false
                                } else {
                                    let check_board = (reach1 | reach2)
                                        & !(prelude.oppo_workers
                                            | end_masks
                                            | prelude.domes_and_frozen)
                                        & prelude.exactly_level_3
                                        & prelude.win_mask;
                                    check_board.is_not_empty()
                                }
                            };

                            result.push(build_scored_move::<F, _>(
                                new_action,
                                is_check,
                                end_height_1 > start_height_2 || end_height_2 > start_height_2,
                            ));
                        }
                    }
                }
            } else {
                // TODO: single move only
            }
        }
    }

    if is_mate_only::<F>() || MUST_CLIMB {
        return result;
    }

    let unblocked_squares = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);

    let mut own_workers = prelude.own_workers.into_iter();
    let worker_start_1 = own_workers.next().unwrap();

    // Double builds
    let worker_start_state = get_worker_start_move_state(&prelude, worker_start_1);

    let possible_builds_1 =
        NEIGHBOR_MAP[worker_start_1 as usize] & unblocked_squares & prelude.build_mask;

    let mut reach = if worker_start_state.worker_start_height == 2 {
        WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx][worker_start_1 as usize]
    } else {
        BitBoard::EMPTY
    };

    if let Some(worker_start_2) = worker_start_state.other_own_workers.into_iter().next() {
        if prelude.is_against_hypnus
            && (prelude.own_workers & prelude.exactly_level_2) != prelude.own_workers
        {
            reach = BitBoard::EMPTY;
        } else {
            if prelude.board.get_height(worker_start_2) == 2 {
                reach |= WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx][worker_start_2 as usize];
            }
            reach &= unblocked_squares & prelude.win_mask;
        }

        let possible_builds_2 =
            NEIGHBOR_MAP[worker_start_2 as usize] & unblocked_squares & prelude.build_mask;

        let overlap = possible_builds_1 & possible_builds_2;
        let not_overlap = !overlap;

        for b1 in possible_builds_1 {
            let b1_mask = BitBoard::as_mask(b1);

            let b2_builds =
                if is_interact_with_key_squares::<F>() && (key_squares & b1_mask).is_empty() {
                    possible_builds_2 & key_squares
                } else {
                    possible_builds_2 & !(prelude.exactly_level_3 & b1_mask)
                };

            for b2 in b2_builds {
                let b2_mask = BitBoard::as_mask(b2);
                let both_mask = b1_mask | b2_mask;
                if (both_mask & not_overlap).is_empty() {
                    if (b2 as u8) > (b1 as u8) {
                        continue;
                    }
                }

                let is_check = {
                    let final_lvl_3 = if b1 == b2 {
                        (prelude.exactly_level_3 & !both_mask)
                            | (prelude.exactly_level_1 & both_mask)
                    } else {
                        (prelude.exactly_level_3 & !both_mask)
                            | (prelude.exactly_level_2 & both_mask)
                    };
                    (final_lvl_3 & reach).is_not_empty()
                };
                let new_action = CastorMove::new_double_build(b1, b2);
                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }
    } else {
        reach &= unblocked_squares & prelude.win_mask;

        let narrowed_builds = if is_interact_with_key_squares::<F>() {
            possible_builds_1 & key_squares
        } else {
            possible_builds_1
        };

        for b1 in narrowed_builds {
            let b1_mask = BitBoard::as_mask(b1);
            let is_check = {
                let final_lvl_3 =
                    (prelude.exactly_level_3 & !b1_mask) | (prelude.exactly_level_2 & b1_mask);

                (final_lvl_3 & reach).is_not_empty()
            };
            let new_action = CastorMove::new_single_build(b1);
            result.push(build_scored_move::<F, _>(new_action, is_check, false));
        }
    }

    result
}

pub const fn build_castor() -> GodPower {
    god_power(
        GodName::Castor,
        build_god_power_movers!(castor_move_gen),
        build_god_power_actions::<CastorMove>(),
        2979614850588903286,
        362356524330526493,
    )
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::fen::parse_fen;

    use super::*;

    #[test]
    fn test_castor_wins_move_out_of_eachothers_way_1() {
        let fen = "0000000000002300000000000/1/castor:C3,D3/mortal:A1,B1";
        let state = parse_fen(fen).unwrap();
        let castor = GodName::Castor.to_power();

        let next_moves = castor.get_moves_for_search(&state, Player::One);
        for m in next_moves {
            if m.action.get_is_winning() {
                return;
            }
        }

        assert!(false, "Could not find expected win");
    }

    #[test]
    fn test_castor_wins_move_out_of_eachothers_way_2() {
        let fen = "0000000000003200000000000/1/castor:C3,D3/mortal:A1,B1";
        let state = parse_fen(fen).unwrap();
        let castor = GodName::Castor.to_power();

        let next_moves = castor.get_moves_for_search(&state, Player::One);
        for m in next_moves {
            if m.action.get_is_winning() {
                return;
            }
        }

        assert!(false, "Could not find expected win");
    }

    #[test]
    fn test_castor_debug() {
        let fen = "0000000000000000000000000/1/castor:D5,A3/castor:C4";
        let state = parse_fen(fen).unwrap();
        let castor = GodName::Castor.to_power();

        let next_moves = castor.get_moves_for_search(&state, Player::One);
        for m in next_moves {
            let action = m.action;
            if !action.get_is_winning() {
                continue;
            }
            let next_state = state.next_state(castor, action);
            next_state.print_to_console();
            eprintln!("{} / {:0b}", castor.stringify_move(action), action.0);
        }
    }
}
