use crate::{
    bitboard::{
        BitBoard, BitboardMapping, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP, PUSH_MAPPING,
        apply_mapping_to_mask,
    },
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
            GeneratorPreludeState, build_scored_move,
            get_basic_moves_from_raw_data_with_custom_blockers_no_affinity,
            get_generator_prelude_state, get_reverse_direction_neighbor_map,
            get_standard_reach_board_from_parts, get_worker_end_move_state,
            get_worker_next_build_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers, push_winning_moves,
            restrict_moves_by_affinity_area,
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
struct CharonMove(pub MoveData);

impl Into<GenericMove> for CharonMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for CharonMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl CharonMove {
    fn new_charon_basic_move(
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

    fn new_charon_flip_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        flip_from_position: Square,
        flip_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << CHARON_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << CHARON_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << CHARON_BUILD_POSITION_OFFSET)
            | ((flip_from_position as MoveData) << CHARON_FLIP_FROM_POSITION_OFFSET)
            | ((flip_to_position as MoveData) << CHARON_FLIP_TO_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << CHARON_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << CHARON_MOVE_TO_POSITION_OFFSET)
            | ((25 as MoveData) << CHARON_FLIP_FROM_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn new_charon_winning_flip_move(
        move_from_position: Square,
        move_to_position: Square,
        flip_from_position: Square,
        flip_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << CHARON_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << CHARON_MOVE_TO_POSITION_OFFSET)
            | ((flip_from_position as MoveData) << CHARON_FLIP_FROM_POSITION_OFFSET)
            | ((flip_to_position as MoveData) << CHARON_FLIP_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> CHARON_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn maybe_flip_from_position(&self) -> Option<Square> {
        let value = (self.0 >> CHARON_FLIP_FROM_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn flip_to_position(&self) -> Square {
        Square::from((self.0 >> CHARON_FLIP_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for CharonMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            if let Some(flip_from) = self.maybe_flip_from_position() {
                let flip_to = self.flip_to_position();
                write!(f, "({}>{}){}>{}#", flip_from, flip_to, move_from, move_to)
            } else {
                write!(f, "{}>{}#", move_from, move_to)
            }
        } else {
            if let Some(flip_from) = self.maybe_flip_from_position() {
                let flip_to = self.flip_to_position();
                write!(
                    f,
                    "({}>{}){}>{}^{}",
                    flip_from, flip_to, move_from, move_to, build
                )
            } else {
                write!(f, "{}>{}^{}", move_from, move_to, build)
            }
        }
    }
}

impl GodMove for CharonMove {
    fn move_to_actions(self, _board: &BoardState, _player: Player, _other_god: StaticGod) -> Vec<FullAction> {
        let mut result = vec![];

        result.push(PartialAction::SelectWorker(self.move_from_position()));
        if let Some(flip_from) = self.maybe_flip_from_position() {
            result.push(PartialAction::ForceOpponentWorker(
                flip_from,
                self.flip_to_position(),
            ));
        }
        result.push(PartialAction::MoveWorker(self.move_to_position().into()));

        if !self.get_is_winning() {
            result.push(PartialAction::Build(self.build_position()));
        }

        return vec![result];
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        let move_from = BitBoard::as_mask(self.move_from_position());
        let move_to = BitBoard::as_mask(self.move_to_position());
        board.worker_xor(player, move_to ^ move_from);

        if let Some(flip_from) = self.maybe_flip_from_position() {
            let flip_to = self.flip_to_position();

            board.oppo_worker_xor(
                other_god,
                !player,
                flip_from.to_board() ^ flip_to.to_board(),
            );
        }

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        if let Some(flip_from) = self.maybe_flip_from_position() {
            flip_from.to_board()
                | self.flip_to_position().to_board()
                | self.move_from_position().to_board()
                | self.move_to_position().to_board()
        } else {
            self.move_from_position().to_board() | self.move_to_position().to_board()
        }
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_maybe_square_with_height(board, self.maybe_flip_from_position());
        helper.get()
    }
}

fn _is_check(
    prelude: &GeneratorPreludeState,
    build_pos_mask: BitBoard,
    reverse_neighbor_map: &BitboardMapping,
    reach_board: BitBoard,
    final_oppo_workers: BitBoard,
    final_threatening_workers: BitBoard,
    open_board: BitBoard,
) -> bool {
    let final_level_3 =
        (prelude.exactly_level_2 & build_pos_mask) | (prelude.exactly_level_3 & !build_pos_mask);
    let check_board = reach_board & final_level_3;

    if (check_board & !final_oppo_workers).is_not_empty() {
        return true;
    }

    let needs_flipping_checks = check_board & final_oppo_workers;

    for needs_flipping_check_pos in needs_flipping_checks {
        for flip_from_worker in
            reverse_neighbor_map[needs_flipping_check_pos as usize] & final_threatening_workers
        {
            let flip_to_spot =
                PUSH_MAPPING[needs_flipping_check_pos as usize][flip_from_worker as usize];
            if let Some(flip_to_spot) = flip_to_spot {
                if (open_board & flip_to_spot.to_board()).is_not_empty() {
                    return true;
                }
            }
        }
    }

    return false;
}

pub(super) fn charon_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(charon_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let reverse_neighbor_map = get_reverse_direction_neighbor_map(&prelude);
    let flippable_oppo_workers = state.board.workers[!player as usize] & !prelude.domes_and_frozen;

    let all_starting_blocked_squares =
        prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let non_oppo_worker_blockers =
            worker_start_state.other_own_workers | prelude.domes_and_frozen;

        let other_threatening_workers = worker_start_state.other_own_workers & checkable_mask;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &prelude.standard_neighbor_map);

        let mut base_moves_no_affinity_or_oppo_workers =
            get_basic_moves_from_raw_data_with_custom_blockers_no_affinity::<MUST_CLIMB>(
                &prelude,
                worker_start_state.worker_start_pos,
                worker_start_state.worker_start_height,
                non_oppo_worker_blockers,
            );

        let mut mortal_moves = restrict_moves_by_affinity_area(
            worker_start_state.worker_start_mask,
            base_moves_no_affinity_or_oppo_workers & !prelude.oppo_workers,
            prelude.affinity_area,
        );

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let mortal_moves_to_level_3 = mortal_moves & prelude.exactly_level_3 & prelude.win_mask;

            if push_winning_moves::<F, CharonMove, _>(
                &mut result,
                worker_start_pos,
                mortal_moves_to_level_3,
                CharonMove::new_winning_move,
            ) {
                return result;
            }

            mortal_moves ^= mortal_moves_to_level_3;

            // If we can win without flipping, then don't let user win by flipping
            // Yeah it's technically allowed but whatever
            base_moves_no_affinity_or_oppo_workers ^= mortal_moves_to_level_3;
        }

        if !is_mate_only::<F>() {
            for worker_move_pos in mortal_moves {
                let worker_end_move_state =
                    get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_move_pos);
                let worker_next_build_state = get_worker_next_build_state::<F>(
                    &prelude,
                    &worker_start_state,
                    &worker_end_move_state,
                );

                let unblocked_except_oppo_workers = !(prelude.domes_and_frozen
                    | worker_start_state.other_own_workers
                    | worker_end_move_state.worker_end_mask);
                let reach_board = get_standard_reach_board_from_parts::<F>(
                    &prelude,
                    other_threatening_workers,
                    other_threatening_neighbors,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.is_now_lvl_2,
                    unblocked_except_oppo_workers,
                );

                let final_threatening_workers = other_threatening_workers
                    | (BitBoard::CONDITIONAL_MASK[worker_end_move_state.is_now_lvl_2 as usize]
                        & worker_end_move_state.worker_end_mask);

                for worker_build_pos in worker_next_build_state.narrowed_builds {
                    let build_pos_mask = worker_build_pos.to_board();
                    let new_action = CharonMove::new_charon_basic_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                    );

                    let is_check = _is_check(
                        &prelude,
                        build_pos_mask,
                        reverse_neighbor_map,
                        reach_board,
                        prelude.oppo_workers,
                        final_threatening_workers,
                        unblocked_except_oppo_workers
                            & !(prelude.oppo_workers | prelude.exactly_level_3 & build_pos_mask),
                    );

                    result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        worker_end_move_state.is_improving,
                    ))
                }
            }
        }

        if is_mate_only::<F>() {
            if (base_moves_no_affinity_or_oppo_workers & prelude.exactly_level_3).is_empty() {
                continue;
            }
        }

        let mut possible_flips = NEIGHBOR_MAP[worker_start_pos as usize] & flippable_oppo_workers;
        if is_mate_only::<F>() {
            if prelude.other_god.is_aphrodite {
                // If we're against aphrodite and only looking for mates, only bother checking if there's actually level 3's available
                if (base_moves_no_affinity_or_oppo_workers & prelude.exactly_level_3).is_empty() {
                    continue;
                }
            } else {
                // Unless we're against Aphrodite, only consider flips that actually open up level 3 squares
                possible_flips &= prelude.exactly_level_3;
            }
        }

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
            let all_blockers_after_flip = non_oppo_worker_blockers | new_oppo_workers;
            let unblocked_squares_after_flip = !all_blockers_after_flip;

            let mut moves_after_flip =
                base_moves_no_affinity_or_oppo_workers & unblocked_squares_after_flip;

            if prelude.other_god.is_aphrodite {
                let new_affinity_area =
                    apply_mapping_to_mask(new_oppo_workers, &INCLUSIVE_NEIGHBOR_MAP);
                moves_after_flip = restrict_moves_by_affinity_area(
                    worker_start_state.worker_start_mask,
                    moves_after_flip,
                    new_affinity_area,
                );
            }

            if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
                let moves_to_level_3 =
                    moves_after_flip & prelude.exactly_level_3 & prelude.win_mask;

                for worker_end_pos in moves_to_level_3 {
                    let new_action = CharonMove::new_charon_winning_flip_move(
                        worker_start_pos,
                        worker_end_pos,
                        flip_start_pos,
                        flip_dest,
                    );
                    result.push(ScoredMove::new_winning_move(new_action.into()));
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                }

                moves_after_flip ^= moves_to_level_3;
            }

            if is_mate_only::<F>() {
                continue;
            }

            let new_build_mask =
                prelude.other_god.get_build_mask(new_oppo_workers) | prelude.exactly_level_3;

            for worker_move_pos in moves_after_flip {
                let worker_end_move_state =
                    get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_move_pos);

                let all_possible_builds = NEIGHBOR_MAP
                    [worker_end_move_state.worker_end_pos as usize]
                    & unblocked_squares_after_flip
                    & new_build_mask;

                let mut narrowed_builds = all_possible_builds;
                if is_interact_with_key_squares::<F>() {
                    let interact_board = key_squares
                        & (worker_end_move_state.worker_end_mask
                            | flip_start_mask
                            | flip_dest_mask);

                    if interact_board.is_empty() {
                        narrowed_builds &= prelude.key_squares;
                    }
                }

                let unblocked_except_oppo_workers = !(prelude.domes_and_frozen
                    | worker_start_state.other_own_workers
                    | worker_end_move_state.worker_end_mask);
                let reach_board = get_standard_reach_board_from_parts::<F>(
                    &prelude,
                    other_threatening_workers,
                    other_threatening_neighbors,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.is_now_lvl_2,
                    unblocked_except_oppo_workers,
                );

                let final_threatening_workers = other_threatening_workers
                    | (BitBoard::CONDITIONAL_MASK[worker_end_move_state.is_now_lvl_2 as usize]
                        & worker_end_move_state.worker_end_mask);

                for worker_build_pos in narrowed_builds {
                    let worker_build_pos_mask = worker_build_pos.to_board();

                    let new_action = CharonMove::new_charon_flip_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                        flip_start_pos,
                        flip_dest,
                    );

                    let is_check = _is_check(
                        &prelude,
                        worker_build_pos_mask,
                        reverse_neighbor_map,
                        reach_board,
                        new_oppo_workers,
                        final_threatening_workers,
                        unblocked_except_oppo_workers
                            & !(new_oppo_workers | prelude.exactly_level_3 & worker_build_pos_mask),
                    );

                    result.push(build_scored_move::<F, _>(
                        new_action,
                        is_check,
                        worker_end_move_state.is_improving,
                    ));
                }
            }
        }
    }

    result
}

pub const fn build_charon() -> GodPower {
    god_power(
        GodName::Charon,
        build_god_power_movers!(charon_move_gen),
        build_god_power_actions::<CharonMove>(),
        15324631767000384691,
        2986174260566155220,
    )
}
