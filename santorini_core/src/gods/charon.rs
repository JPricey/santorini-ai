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
            get_active_portal, get_active_portal_after_displacement,
            get_basic_moves_from_raw_data_with_custom_blockers_no_affinity,
            get_generator_prelude_state, get_reverse_direction_neighbor_map,
            get_standard_reach_board_from_parts, get_worker_end_move_state,
            get_worker_next_build_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers,
            push_winning_moves, put_moves_through_portals,
            restrict_moves_by_affinity_area, winnable_squares_for_arrival,
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
    let checkable_mask = prelude.mate_start_mask;
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

        // Raw entries, not portal-swapped. Charon's flip happens *before* he moves, so it can arm a
        // portal by pulling a worker off a whirlpool, or disarm one by pulling a worker onto it.
        // Whether the teleport is available therefore depends on which flip he picks, and the swap
        // has to be re-applied per flip against post-flip occupancy rather than baked in here.
        let base_entries_no_affinity_or_oppo_workers =
            get_basic_moves_from_raw_data_with_custom_blockers_no_affinity::<MUST_CLIMB, false>(
                &prelude,
                worker_start_state.worker_start_pos,
                worker_start_state.worker_start_mask,
                worker_start_state.worker_start_height,
                non_oppo_worker_blockers,
            );

        // With no flip nothing is displaced, so this is the ordinary lone-mover portal - the same
        // one `worker_start_state.winnable_squares` was built from.
        let no_flip_portal = get_active_portal(&prelude, worker_start_state.worker_start_mask);

        let mut mortal_moves = restrict_moves_by_affinity_area(
            worker_start_state.worker_start_mask,
            put_moves_through_portals(
                base_entries_no_affinity_or_oppo_workers & !prelude.oppo_workers,
                no_flip_portal,
            ),
            prelude.affinity_area,
        );

        // Squares he can already win on without flipping anybody. Kept in *outcome* space, and
        // subtracted per flip below rather than removed from the base: the base is in entry space,
        // and the same entry can lead somewhere else entirely under a flip that disarms the portal.
        let mut no_flip_winning_outcomes = BitBoard::EMPTY;

        if is_mate_only::<F>() || worker_start_state.can_mate {
            let mortal_moves_to_level_3 = mortal_moves & worker_start_state.winnable_squares;

            if push_winning_moves::<F, CharonMove, _>(
                &mut result,
                worker_start_pos,
                mortal_moves_to_level_3,
                CharonMove::new_winning_move,
            ) {
                return result;
            }

            mortal_moves ^= mortal_moves_to_level_3;
            no_flip_winning_outcomes = mortal_moves_to_level_3;
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
                    worker_end_move_state.is_mate_capable,
                    unblocked_except_oppo_workers,
                );

                let final_threatening_workers = other_threatening_workers
                    | (BitBoard::CONDITIONAL_MASK[worker_end_move_state.is_mate_capable as usize]
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
            if (base_entries_no_affinity_or_oppo_workers
                & (prelude.exactly_level_3 | prelude.portal_squares))
                .is_empty()
            {
                continue;
            }
        }

        let mut possible_flips = NEIGHBOR_MAP[worker_start_pos as usize] & flippable_oppo_workers;
        if is_mate_only::<F>() {
            if prelude.other_god.is_aphrodite {
                // If we're against aphrodite and only looking for mates, only bother checking if there's actually level 3's available
                if (base_entries_no_affinity_or_oppo_workers
                    & (prelude.exactly_level_3 | prelude.portal_squares))
                    .is_empty()
                {
                    continue;
                }
            } else {
                // Unless we're against Aphrodite, only consider flips that actually open up a square
                // worth winning on. That is normally just the level 3 squares, but pulling a worker
                // off a *whirlpool* arms the portal, and the mate is then stepping into that
                // whirlpool - from any height - and surfacing on a level 3 exit.
                possible_flips &= prelude.exactly_level_3 | prelude.portal_squares;
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

            // The flip resolves before Charon moves, so the portal he arrives into is the one left
            // standing afterwards: dragging a worker off a whirlpool arms it, dragging one onto a
            // whirlpool disarms it. Both directions change which moves exist and which of them win.
            let portal_after_flip = get_active_portal_after_displacement(
                &prelude,
                worker_start_state.worker_start_mask | flip_start_mask,
                flip_dest_mask,
            );

            let mut moves_after_flip = put_moves_through_portals(
                base_entries_no_affinity_or_oppo_workers & unblocked_squares_after_flip,
                portal_after_flip,
            );

            if prelude.other_god.is_aphrodite {
                let new_affinity_area =
                    apply_mapping_to_mask(new_oppo_workers, &INCLUSIVE_NEIGHBOR_MAP);
                moves_after_flip = restrict_moves_by_affinity_area(
                    worker_start_state.worker_start_mask,
                    moves_after_flip,
                    new_affinity_area,
                );
            }

            if is_mate_only::<F>() || worker_start_state.can_mate {
                let winning_outcomes = moves_after_flip
                    & winnable_squares_for_arrival(
                        &prelude,
                        worker_start_state.worker_start_height,
                        portal_after_flip,
                    );

                // Every winning square leaves the quiet set, but only the ones the flip actually
                // opened up are emitted - winning here what he could already win without flipping
                // is legal, just pointless.
                let moves_to_level_3 = winning_outcomes & !no_flip_winning_outcomes;

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

                moves_after_flip ^= winning_outcomes;
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
                    worker_end_move_state.is_mate_capable,
                    unblocked_except_oppo_workers,
                );

                let final_threatening_workers = other_threatening_workers
                    | (BitBoard::CONDITIONAL_MASK[worker_end_move_state.is_mate_capable as usize]
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

#[cfg(test)]
mod charybdis_portal_tests {
    use crate::board::{GameStateBuilder, GodData};
    use crate::gods::GodName;
    use crate::player::Player;
    use crate::square::Square::*;

    use super::*;

    fn with_whirlpools(
        mut state: FullGameState,
        player: Player,
        squares: &[Square],
    ) -> FullGameState {
        let mut mask = BitBoard::EMPTY;
        for s in squares {
            mask |= BitBoard::as_mask(*s);
        }
        state.board.set_god_data(player, mask.0 as GodData);
        state
    }

    /// Charon's pull resolves before he moves, so pulling a worker off a whirlpool arms the portal
    /// within the same turn - and the mate that opens up starts from a worker that cannot climb.
    #[test]
    fn test_charon_arms_the_portal_by_pulling_a_worker_off_it() {
        // Whirlpools C5 and B2. C5 is held by a Charybdis worker, so the portal is shut. Charon at
        // D4 pulls C5 through to E3, freeing C5, then steps onto it and surfaces on B2 at level 3.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Charon)
                .with_p1_worker(C5)
                .with_p1_worker(A5)
                .with_p2_worker(D4)
                .with_p2_worker(A1)
                .with_height(B2, 3)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C5, B2],
        );

        let charon = GodName::Charon.to_power();
        let wins = charon.get_winning_moves(&state, Player::Two);

        let mut found = false;
        for scored in &wins {
            let m: CharonMove = scored.action.into();
            if m.move_from_position() == D4
                && m.move_to_position() == B2
                && m.maybe_flip_from_position() == Some(C5)
            {
                found = true;
                let next =
                    state.next_state(charon, GodName::Charybdis.to_power(), scored.action);
                assert_eq!(next.get_winner(), Some(Player::Two));
                assert!(next.board.workers[Player::Two as usize].contains_square(B2));
            }
        }
        assert!(
            found,
            "expected Charon to pull C5 free, enter it, and surface on B2. Wins: {:?}",
            wins.iter()
                .map(|m| charon.stringify_move(m.action))
                .collect::<Vec<_>>()
        );
    }

    /// The mirror case: a pull that parks a worker *onto* a whirlpool shuts the portal down for
    /// Charon's own move, so the teleport that would otherwise be available is not generated.
    #[test]
    fn test_charon_disarms_the_portal_by_pulling_a_worker_onto_it() {
        // Whirlpools C4 and E2, both free, so the portal is armed. Charon at C3 can pull the
        // Charybdis worker at C2 through to C4, which shuts the portal.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Charon)
                .with_p1_worker(C2)
                .with_p1_worker(A5)
                .with_p2_worker(C3)
                .with_p2_worker(A1)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, E2],
        );

        let charon = GodName::Charon.to_power();
        for scored in charon.get_all_moves(&state, Player::Two) {
            let m: CharonMove = scored.action.into();
            if m.maybe_flip_from_position() != Some(C2) {
                continue;
            }
            // C4 is filled by the pulled worker, so it is neither enterable nor a usable exit:
            // no move on this flip may land on either whirlpool.
            assert_ne!(
                m.move_to_position(),
                C4,
                "C4 is occupied by the pulled worker"
            );
            assert_ne!(
                m.move_to_position(),
                E2,
                "the portal is shut, so nothing should surface on E2"
            );
        }
    }

    /// A flip that shuts the portal changes where an entry *leads*, so the entry stays legal even
    /// when the same square would have been a winning teleport without the flip. Removing it
    /// wholesale (which the entry-space bookkeeping used to do) silently loses this move.
    #[test]
    fn test_charon_may_still_enter_a_whirlpool_whose_exit_his_flip_just_blocked() {
        // Whirlpools C2 and D3, both free, so C3 -> C2 would surface on D3 and win. Flipping B3
        // through to D3 parks a worker on the exit, so the same step onto C2 just lands on C2.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Charon)
                .with_p1_worker(B3)
                .with_p1_worker(A5)
                .with_p2_worker(C3)
                .with_p2_worker(A1)
                .with_height(D3, 3)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C2, D3],
        );

        let charon = GodName::Charon.to_power();

        // Without a flip it is a portal win.
        assert!(
            charon
                .get_winning_moves(&state, Player::Two)
                .iter()
                .any(|m| {
                    let m: CharonMove = m.action.into();
                    m.move_from_position() == C3
                        && m.move_to_position() == D3
                        && m.maybe_flip_from_position().is_none()
                }),
            "expected the no-flip portal win onto D3"
        );

        // With the flip blocking the exit, stepping onto C2 is an ordinary quiet move.
        let found = charon.get_all_moves(&state, Player::Two).iter().any(|m| {
            let m: CharonMove = m.action.into();
            !m.get_is_winning()
                && m.maybe_flip_from_position() == Some(B3)
                && m.move_from_position() == C3
                && m.move_to_position() == C2
        });
        assert!(
            found,
            "flipping B3 onto D3 shuts the portal, so C3 -> C2 has to stay available"
        );
    }
}
