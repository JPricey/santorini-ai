use crate::{
    bitboard::{
        BETWEEN_MAPPING, BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK, NEIGHBOR_MAP,
        apply_mapping_to_mask,
    },
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            ANY_MOVE_FILTER, GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK,
            MoveData, MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::prometheus_slide,
        move_helpers::{
            WorkerNextMoveState, build_scored_move, get_generator_prelude_state, get_sized_result,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const PRE_BUILD_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

const NO_PRE_BUILD_VALUE: MoveData = 25 << PRE_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AchillesMove(pub MoveData);

impl Into<GenericMove> for AchillesMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AchillesMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AchillesMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | NO_PRE_BUILD_VALUE;

        Self(data)
    }

    fn new_power_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        pre_build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((pre_build_position as MoveData) << PRE_BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | NO_PRE_BUILD_VALUE
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn new_power_winning_move(
        move_from_position: Square,
        move_to_position: Square,
        pre_build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((pre_build_position as MoveData) << PRE_BUILD_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub(crate) fn move_to_position(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn pre_build_position(self) -> Option<Square> {
        let value = (self.0 >> PRE_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
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

impl std::fmt::Debug for AchillesMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            if let Some(pre_build) = self.pre_build_position() {
                write!(f, "^{} {}>{}#", pre_build, move_from, move_to)
            } else {
                write!(f, "{}>{}#", move_from, move_to)
            }
        } else if let Some(pre_build) = self.pre_build_position() {
            write!(f, "^{} {}>{}^{}", pre_build, move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for AchillesMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let move_from = self.move_from_position();
        let move_to = self.move_to_position();

        if self.get_is_winning() {
            if let Some(pre_build_position) = self.pre_build_position() {
                return vec![vec![
                    PartialAction::Build(pre_build_position),
                    PartialAction::SelectWorker(move_from),
                    PartialAction::MoveWorker(move_to.into()),
                ]];
            } else {
                return vec![vec![
                    PartialAction::SelectWorker(move_from),
                    PartialAction::MoveWorker(move_to.into()),
                ]];
            }
        }

        let build_position = self.build_position();

        if let Some(pre_build_position) = self.pre_build_position() {
            let mut res = vec![vec![
                PartialAction::Build(pre_build_position),
                PartialAction::SelectWorker(move_from),
                PartialAction::MoveWorker(move_to.into()),
                PartialAction::Build(build_position),
            ]];

            let from_neighbors = NEIGHBOR_MAP[move_from as usize];
            let to_neighbors = NEIGHBOR_MAP[move_to as usize];
            let both_neighbors = from_neighbors & to_neighbors;

            // Harpies can push the worker through a square on its way to move_to. Building on
            // that square before the move can block the step onto it or change where the push
            // ends, so a build there is not interchangeable. Pushes of more than one square
            // leave nothing adjacent to both ends, so the single between square covers it.
            let pushed_through_mask = BETWEEN_MAPPING[move_from as usize][move_to as usize]
                .map_or(BitBoard::EMPTY, BitBoard::as_mask);

            let pre_build_mask = BitBoard::as_mask(pre_build_position);
            let build_mask = BitBoard::as_mask(build_position);
            let are_builds_interchangeable = (both_neighbors & pre_build_mask).is_not_empty()
                && (both_neighbors & build_mask).is_not_empty()
                && ((pre_build_mask | build_mask) & pushed_through_mask).is_empty();

            if are_builds_interchangeable {
                res.push(vec![
                    PartialAction::Build(build_position),
                    PartialAction::SelectWorker(move_from),
                    PartialAction::MoveWorker(move_to.into()),
                    PartialAction::Build(pre_build_position),
                ]);
            }

            res
        } else {
            vec![vec![
                PartialAction::SelectWorker(move_from),
                PartialAction::MoveWorker(move_to.into()),
                PartialAction::Build(build_position),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if let Some(pre_build_position) = self.pre_build_position() {
            board.set_god_data(player, 1);
            board.build_up(pre_build_position);
        }

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_maybe_square_with_height(board, self.pre_build_position());
        helper.get()
    }
}

fn _achilles_must_climb_not_using_power_but_has_power_available<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) -> bool {
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let check_if_build_on = prelude.exactly_level_1 | prelude.exactly_level_2;
    let check_if_not_build_on =
        prelude.exactly_level_3 | (prelude.exactly_level_2 & prelude.build_mask);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let worker_next_moves =
            get_worker_next_move_state::<true>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, AchillesMove, _>(
                result,
                worker_start_pos,
                moves_to_level_3,
                AchillesMove::new_winning_move,
            ) {
                return true;
            }
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);

        let climbing_moves = worker_next_moves.worker_moves & !prelude.exactly_level_3;
        for worker_end_pos in climbing_moves {
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
                    worker_moves: worker_next_moves.worker_moves,
                },
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = AchillesMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 =
                        (check_if_build_on & build_mask) | (check_if_not_build_on & !build_mask);
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

    false
}

fn _achilles_must_climb_using_power<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) {
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let worker_neighbors = NEIGHBOR_MAP[worker_start_pos as usize];
        let unblocked_squares =
            !(worker_start_state.all_non_moving_workers | prelude.domes_and_frozen);

        let all_prebuilds = worker_neighbors & unblocked_squares;

        let (same_height, above_height) = match worker_start_state.worker_start_height {
            0 => (
                all_prebuilds & prelude.exactly_level_0,
                all_prebuilds & prelude.exactly_level_1,
            ),
            1 => (
                all_prebuilds & prelude.exactly_level_1,
                all_prebuilds & prelude.exactly_level_2,
            ),
            2 => (
                all_prebuilds & prelude.exactly_level_2,
                all_prebuilds & prelude.exactly_level_3,
            ),
            3 => (BitBoard::EMPTY, BitBoard::EMPTY),
            _ => unreachable!(),
        };

        let mut same_height_allowed_builds = same_height;
        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            for pre_build_pos in same_height {
                let winning_move = AchillesMove::new_power_winning_move(
                    worker_start_pos,
                    pre_build_pos,
                    pre_build_pos,
                );
                result.push(build_scored_move::<F, _>(winning_move, false, false));
                if is_stop_on_mate::<F>() {
                    return;
                }
            }

            if is_mate_only::<F>() {
                continue;
            }

            same_height_allowed_builds ^= same_height;
        }

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);

        // Use power then step onto that square
        for worker_end_pos in same_height_allowed_builds {
            let end_mask = worker_end_pos.to_board();
            let end_height = prelude.board.get_height(worker_end_pos) + 1;
            let is_now_lvl_2 = (end_height == 2) as usize;

            let worker_end_neighbors = NEIGHBOR_MAP[worker_end_pos as usize];

            let reach_board = (other_threatening_neighbors
                | (worker_end_neighbors & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                & (unblocked_squares ^ end_mask);

            let mut worker_builds = worker_end_neighbors & unblocked_squares;
            if is_interact_with_key_squares::<F>() {
                if (end_mask & key_squares).is_empty() {
                    worker_builds &= key_squares;
                }
            }

            for build_pos in worker_builds {
                let build_mask = build_pos.to_board();

                let is_check = {
                    let final_level_3 = prelude.exactly_level_2 & build_mask
                        | prelude.exactly_level_3 & !build_mask;
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                let new_action = AchillesMove::new_power_move(
                    worker_start_pos,
                    worker_end_pos,
                    build_pos,
                    worker_end_pos,
                );
                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }

        // Move then use power elsewhere
        for worker_end_pos in above_height {
            let end_mask = worker_end_pos.to_board();
            let end_height = prelude.board.get_height(worker_end_pos);
            let is_now_lvl_2 = (end_height == 2) as usize;

            let allowed_prebuilds = all_prebuilds ^ end_mask;
            let worker_end_neighbors = NEIGHBOR_MAP[worker_end_pos as usize];

            let reach_board = (other_threatening_neighbors
                | (worker_end_neighbors & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                & (unblocked_squares ^ end_mask);

            let post_build_locations = worker_end_neighbors & unblocked_squares;

            for pre_build_pos in allowed_prebuilds {
                let pre_build_mask = pre_build_pos.to_board();

                // Can't dome the power build square and then build on it again
                let mut worker_builds =
                    post_build_locations & !(prelude.exactly_level_3 & pre_build_mask);

                // An interchangeable pair of builds reaches the same position in either order, so
                // only keep the ordering where the power build square sorts last. That square has
                // to be buildable after the move for this, otherwise the swapped ordering never
                // gets generated and dropping this one loses the move.
                if (post_build_locations & pre_build_mask).is_not_empty() {
                    let both_buildable = worker_builds & allowed_prebuilds;
                    worker_builds ^=
                        both_buildable & LOWER_SQUARES_EXCLUSIVE_MASK[pre_build_pos as usize];
                }

                if is_interact_with_key_squares::<F>() {
                    if ((pre_build_mask | end_mask) & key_squares).is_empty() {
                        worker_builds &= key_squares;
                    }
                }

                for build_pos in worker_builds {
                    let build_mask = build_pos.to_board();
                    let is_double_build = pre_build_pos == build_pos;

                    let is_check = {
                        let final_level_3 = if is_double_build {
                            (prelude.exactly_level_1 & pre_build_mask)
                                | (prelude.exactly_level_3 & !pre_build_mask)
                        } else {
                            let both_build_mask = pre_build_mask | build_mask;
                            (prelude.exactly_level_2 & both_build_mask)
                                | (prelude.exactly_level_3 & !both_build_mask)
                        };
                        let check_board = reach_board & final_level_3;
                        check_board.is_not_empty()
                    };

                    let new_action = AchillesMove::new_power_move(
                        worker_start_pos,
                        worker_end_pos,
                        build_pos,
                        pre_build_pos,
                    );
                    result.push(build_scored_move::<F, _>(new_action, is_check, false));
                }
            }
        }
    }
}

fn achilles_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if state.gods[!player as usize].is_harpies() {
        _achilles_move_gen::<F, MUST_CLIMB, true>(state, player, key_squares)
    } else {
        _achilles_move_gen::<F, MUST_CLIMB, false>(state, player, key_squares)
    }
}

fn _achilles_move_gen<
    const F: MoveGenFlags,
    const MUST_CLIMB: bool,
    const AGAINST_HARPIES: bool,
>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let has_power_available = state.board.god_data[player as usize] == 0;
    if !MUST_CLIMB && prelude.other_god.god_name == GodName::Persephone {
        if has_power_available {
            let mut result = get_sized_result::<F>();
            let did_mate = _achilles_must_climb_not_using_power_but_has_power_available::<F>(
                state,
                player,
                key_squares,
                &mut result,
            );
            if is_stop_on_mate::<F>() && did_mate {
                return result;
            }

            if result.len() > 0 {
                // Mortal climbing is possible - add power climbing options too
                _achilles_must_climb_using_power::<F>(state, player, key_squares, &mut result);
                return result;
            }

            // Maybe we couldn't find a move because we were filtering moves somehow
            // Try to find a move without filtering... if we can, return the empty result
            // Otherwise, we'll fall back to not climbing
            if F & ANY_MOVE_FILTER > 0 {
                _achilles_must_climb_not_using_power_but_has_power_available::<0>(
                    state,
                    player,
                    key_squares,
                    &mut result,
                );
                if result.len() > 0 {
                    result.clear();
                    _achilles_must_climb_using_power::<F>(state, player, key_squares, &mut result);
                    return result;
                }
            }

            // No mortal climbing possible - fall through to generate all moves
            // (Achilles is not required to use power to climb)
            result.clear();
        } else {
            let result = achilles_move_gen::<F, true>(state, player, key_squares);
            if result.len() > 0 {
                return result;
            }

            if F & ANY_MOVE_FILTER > 0 {
                let unrestricted = achilles_move_gen::<0, true>(state, player, key_squares);
                if unrestricted.len() > 0 {
                    return vec![];
                }
            }
        }
    }

    let mut result = get_sized_result::<F>();
    if is_mate_only::<F>() && !prelude.can_climb {
        return result;
    }

    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let neighbor_moves_map = prelude.standard_neighbor_map;

    let (check_if_build_on, check_if_not_build_on) = if has_power_available {
        (
            prelude.exactly_level_1 | prelude.exactly_level_2,
            prelude.exactly_level_3 | (prelude.exactly_level_2 & prelude.build_mask),
        )
    } else {
        (prelude.exactly_level_2, prelude.exactly_level_3)
    };

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, AchillesMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                AchillesMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &neighbor_moves_map);

        let unblocked_squares =
            !(worker_start_state.all_non_moving_workers | prelude.domes_and_frozen);

        if has_power_available {
            let mut pre_build_locations =
                NEIGHBOR_MAP[worker_start_pos as usize] & unblocked_squares & prelude.build_mask;

            if is_mate_only::<F>()
                || worker_start_state.worker_start_height == 2 && prelude.can_climb
            {
                let mate_builds = pre_build_locations
                    & prelude.exactly_level_2
                    & prelude.win_mask
                    & worker_next_moves.worker_moves;

                for pre_build_pos in mate_builds {
                    let winning_move = AchillesMove::new_power_winning_move(
                        worker_start_pos,
                        pre_build_pos,
                        pre_build_pos,
                    );
                    result.push(build_scored_move::<F, _>(winning_move, false, false));
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }

                if is_mate_only::<F>() {
                    continue;
                }

                // TODO: technically you should be allowed to not win from here. Whatever.
                pre_build_locations ^= mate_builds;
            }

            // Where the worker may step once the power build has been spent on a given square
            let moves_with_power_build_on = |pre_build_pos: Square| {
                let pre_build_mask = BitBoard::as_mask(pre_build_pos);
                let pre_build_height = prelude.board.get_height(pre_build_pos);

                let mut power_worker_moves = worker_next_moves.worker_moves;
                if pre_build_height >= 3
                    || pre_build_height + (!prelude.can_climb as usize)
                        > worker_start_state.worker_start_height
                {
                    power_worker_moves &= !pre_build_mask;
                } else if prelude.is_down_prevented {
                    match worker_start_state.worker_start_height {
                        1 => power_worker_moves |= pre_build_mask & prelude.exactly_level_0,
                        2 => power_worker_moves |= pre_build_mask & prelude.exactly_level_1,
                        3 => power_worker_moves |= pre_build_mask & prelude.exactly_level_2,
                        _ => (),
                    }
                }

                power_worker_moves
            };

            // Where harpies pushes the worker depends only on its destination, and on whether
            // the power build landed on that destination - not on where else it could have gone.
            // Both outcomes are worth precomputing once per destination rather than once per
            // (power build, destination) pair.
            let mut slide_ends = [(Square::A5, Square::A5); 25];
            if AGAINST_HARPIES {
                for dest in worker_next_moves.worker_moves | pre_build_locations {
                    let dest_height = prelude.board.get_height(dest);
                    slide_ends[dest as usize] = (
                        prometheus_slide(&prelude, worker_start_pos, dest, dest_height),
                        prometheus_slide(&prelude, worker_start_pos, dest, dest_height + 1),
                    );
                }
            }

            for pre_build_pos in pre_build_locations {
                let pre_build_mask = BitBoard::as_mask(pre_build_pos);

                for worker_dest_pos in moves_with_power_build_on(pre_build_pos) {
                    let mut worker_end_pos = worker_dest_pos;
                    let mut worker_end_mask = BitBoard::as_mask(worker_end_pos);
                    let mut worker_end_height = prelude.board.get_height(worker_end_pos)
                        + (worker_end_pos == pre_build_pos) as usize;

                    // Squares that can be built on interchangeably before or after the move.
                    // Only harpies can break that for the destination square, by pushing the
                    // worker on further depending on whether it was built up first.
                    let mut interchangeable_builds = pre_build_locations;

                    if AGAINST_HARPIES {
                        let dest_mask = BitBoard::as_mask(worker_dest_pos);
                        let (plain_end, raised_end) = slide_ends[worker_dest_pos as usize];

                        worker_end_pos = if worker_dest_pos == pre_build_pos {
                            raised_end
                        } else {
                            plain_end
                        };
                        worker_end_mask = BitBoard::as_mask(worker_end_pos);
                        worker_end_height = prelude.board.get_height(worker_end_pos)
                            + (worker_end_pos == pre_build_pos) as usize;

                        // Building on the destination before the move is only interchangeable
                        // with building on it after if the worker gets pushed to the same square
                        // either way, and is allowed to step there either way.
                        let is_dest_interchangeable = plain_end == raised_end
                            && (moves_with_power_build_on(worker_dest_pos)
                                & worker_next_moves.worker_moves
                                & dest_mask)
                                .is_not_empty();

                        if !is_dest_interchangeable {
                            interchangeable_builds &= !dest_mask;
                        }
                    }

                    let is_now_lvl_2 = (worker_end_height == 2) as usize;

                    let post_build_locations = NEIGHBOR_MAP[worker_end_pos as usize]
                        & unblocked_squares
                        & prelude.build_mask
                        & !worker_end_mask;

                    // Can't dome the power build square and then build on it again
                    let mut worker_builds =
                        post_build_locations & !(prelude.exactly_level_3 & pre_build_mask);

                    // An interchangeable pair of builds reaches the same position in either order,
                    // so only keep the ordering where the power build square sorts last. That
                    // square has to be interchangeable itself for this, otherwise the swapped
                    // ordering never gets generated and dropping this one loses the move.
                    if (post_build_locations & interchangeable_builds & pre_build_mask)
                        .is_not_empty()
                    {
                        let both_buildable = worker_builds & interchangeable_builds;
                        worker_builds ^=
                            both_buildable & LOWER_SQUARES_EXCLUSIVE_MASK[pre_build_pos as usize];
                    }

                    let worker_plausible_next_moves =
                        neighbor_moves_map[worker_end_pos as usize] & unblocked_squares;

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
                            | (worker_plausible_next_moves
                                & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                            & prelude.win_mask
                            & !own_final_workers
                    };

                    for worker_build_pos in worker_builds {
                        let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                        let is_double_build = pre_build_pos == worker_build_pos;

                        let new_action = AchillesMove::new_power_move(
                            worker_start_pos,
                            worker_end_pos,
                            worker_build_pos,
                            pre_build_pos,
                        );

                        let is_check = {
                            let final_level_3 = if is_double_build {
                                (prelude.exactly_level_1 & pre_build_mask)
                                    | (prelude.exactly_level_3 & !pre_build_mask)
                            } else {
                                let both_build_mask = pre_build_mask | worker_build_mask;
                                (prelude.exactly_level_2 & both_build_mask)
                                    | (prelude.exactly_level_3 & !both_build_mask)
                            };
                            let check_board = reach_board & final_level_3 & unblocked_squares;
                            check_board.is_not_empty()
                        };

                        result.push(build_scored_move::<F, _>(new_action, is_check, false));
                    }
                }
            }
        }

        if is_mate_only::<F>() {
            continue;
        }

        // Mortal moves
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
                &WorkerNextMoveState {
                    other_threatening_workers,
                    other_threatening_neighbors,
                    worker_moves: worker_next_moves.worker_moves,
                },
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = AchillesMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 =
                        (check_if_build_on & build_mask) | (check_if_not_build_on & !build_mask);
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

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(0),
        "x" | "X" => Ok(1),
        _ => Err(format!("Must be either empty string or x")),
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        _ => Some(format!("x")),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => Some(format!("Power available")),
        _ => Some(format!("Power used")),
    }
}

pub const fn build_achilles() -> GodPower {
    god_power(
        GodName::Achilles,
        build_god_power_movers!(achilles_move_gen),
        build_god_power_actions::<AchillesMove>(),
        4823901567482390156,
        9182736450918273645,
    )
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}

#[cfg(test)]
mod tests {
    use crate::{
        fen::parse_fen, gods::PartialAction, move_verifier::MoveVerifier, square::Square::*,
    };

    #[test]
    fn test_power_build_onto_square_keeps_all_second_builds() {
        let state =
            parse_fen("1000200310000101000100000/1/achilles:B5,E2/hephaestus:D4,D3").unwrap();

        let next_states = state.get_next_states_interactive();

        for second_build in [D1, E1] {
            MoveVerifier::new()
                .with_p1_worker_at(D2)
                .with_height_at(D2, 1)
                .with_height_at(second_build, 1)
                .no_winner()
                .any(&next_states);
        }

        crate::consistency_checker::consistency_check(&state).unwrap();
    }

    // Pre-building on a level-3 neighbour caps it with a dome, so the worker must never be
    // allowed to step onto the square it just power-built. Regression: an h3 worker pre-built
    // a dome on an adjacent h3 square and then moved onto it, landing a worker on a dome.
    #[test]
    fn test_power_build_dome_blocks_stepping_onto_it() {
        let state = parse_fen("2040223404231431344310143/1/achilles:B5,E1/hera:D5,C3").unwrap();

        let next_states = state.get_next_states_interactive();

        // No generated move may leave a worker standing on E2 once it has been domed.
        MoveVerifier::new()
            .with_p1_worker_at(E2)
            .with_height_at(E2, 4)
            .none(&next_states);

        crate::consistency_checker::consistency_check(&state).unwrap();
    }

    // Harpies pushes the C1 worker through D1 on its way to E1. Building D1 first would put it
    // two levels above the worker, so that ordering must not be offered even though D1
    // neighbours both ends of the move.
    #[test]
    fn test_no_interchangeable_build_on_pushed_through_square() {
        let state = parse_fen("0000000000000000000000010/1/achilles:C1,A5/harpies:E5,A1").unwrap();

        let illegal_ordering = vec![
            PartialAction::Build(D1),
            PartialAction::SelectWorker(C1),
            PartialAction::MoveWorker(E1.into()),
            PartialAction::Build(D2),
        ];

        assert!(
            !state
                .get_next_states_interactive()
                .iter()
                .any(|s| s.actions == illegal_ordering)
        );
    }

    // The power build and the post-move build are interchangeable only when both squares can be
    // built on before *and* after the move, so only then may one of the two orderings be dropped.
    // Regression: the ordering was pruned by square index alone, which silently lost every build
    // square shared by the start and end squares whenever the power build square was out of reach
    // after the move.
    #[test]
    fn test_power_build_keeps_shared_build_squares() {
        let state = parse_fen("0000000000000000000000000/1/achilles:C3,A1/mortal:E5,E4").unwrap();

        let next_states = state.get_next_states_interactive();

        // Power build on B4 (only reachable from C3), step C3 -> C2, then build on any neighbour
        // of C2 -- including the squares that neighbour both C3 and C2.
        for second_build in [B3, D3, B2, D2] {
            MoveVerifier::new()
                .with_p1_worker_at(C2)
                .without_p1_worker_at(C3)
                .with_height_at(B4, 1)
                .with_height_at(second_build, 1)
                .no_winner()
                .any(&next_states);
        }

        crate::consistency_checker::consistency_check(&state).unwrap();
    }

    // Same as above, for the separate generator used when persephone forces the climb.
    #[test]
    fn test_power_build_keeps_shared_build_squares_when_forced_to_climb() {
        let state =
            parse_fen("0000000000001000020000000/1/achilles:C3,A1/persephone:E5,E4").unwrap();

        let next_states = state.get_next_states_interactive();

        for second_build in [B3, D3, B2, D2] {
            MoveVerifier::new()
                .with_p1_worker_at(C2)
                .without_p1_worker_at(C3)
                .with_height_at(B4, 1)
                .with_height_at(second_build, 1)
                .no_winner()
                .any(&next_states);
        }

        crate::consistency_checker::consistency_check(&state).unwrap();
    }
}
