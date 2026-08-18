use crate::{
    bitboard::{BETWEEN_MAPPING, BitBoard, NEIGHBOR_MAP, PUSH_MAPPING},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::slide_position,
        move_helpers::{
            build_scored_move, displacer_portal_exit,
            get_basic_moves_from_raw_data_with_custom_blockers_no_portal,
            get_generator_prelude_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MINOTAUR_MOVE_FROM_POSITION_OFFSET: usize = 0;
const MINOTAUR_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const MINOTAUR_BUILD_POSITION_OFFSET: usize = MINOTAUR_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const MINOTAUR_PUSH_TO_POSITION_OFFSET: usize = MINOTAUR_BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct MinotaurMove(pub MoveData);

impl Into<GenericMove> for MinotaurMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for MinotaurMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl MinotaurMove {
    fn new_minotaur_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << MINOTAUR_BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET);

        Self(data)
    }

    fn new_minotaur_push_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        push_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << MINOTAUR_BUILD_POSITION_OFFSET)
            | ((push_to_position as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((25 as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn new_minotaur_winning_push_move(
        move_from_position: Square,
        move_to_position: Square,
        push_to_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << MINOTAUR_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MINOTAUR_MOVE_TO_POSITION_OFFSET)
            | ((push_to_position as MoveData) << MINOTAUR_PUSH_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    fn push_to_position(&self) -> Option<Square> {
        let value = (self.0 >> MINOTAUR_PUSH_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;

        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> MINOTAUR_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn move_mask(self) -> BitBoard {
        self.move_from_position().to_board() | self.move_to_position().to_board()
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for MinotaurMove {
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
        } else if let Some(push_to) = self.push_to_position() {
            write!(f, "{}>{}(>{})^{}", move_from, move_to, push_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for MinotaurMove {
    fn move_to_actions(self, _board: &BoardState, _player: Player, _other_god: StaticGod) -> Vec<FullAction> {
        let mut result = vec![PartialAction::SelectWorker(self.move_from_position())];

        if let Some(push_to) = self.push_to_position() {
            // The pushed worker was on the square between where Minotaur started and where it was
            // shoved to. That is usually where Minotaur lands, but a whirlpool teleports him past
            // it, so read the source off the geometry rather than assuming it is `move_to`.
            let push_from = BETWEEN_MAPPING[self.move_from_position() as usize][push_to as usize]
                .unwrap_or(self.move_to_position());
            result.push(PartialAction::new_move_with_displace(
                self.move_to_position(),
                push_from,
                push_to,
            ));
        } else {
            result.push(PartialAction::MoveWorker(self.move_to_position().into()));
        }

        if !self.get_is_winning() {
            result.push(PartialAction::Build(self.build_position()));
        }

        return vec![result];
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        let move_from = BitBoard::as_mask(self.move_from_position());
        let move_to = BitBoard::as_mask(self.move_to_position());
        board.worker_xor(player, move_to | move_from);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);

        if let Some(push_to) = self.push_to_position() {
            // The pushed worker started on the square between Minotaur's start and its shove
            // destination - `move_to` only when no whirlpool teleported him elsewhere.
            let push_from = BETWEEN_MAPPING[self.move_from_position() as usize][push_to as usize]
                .map(BitBoard::as_mask)
                .unwrap_or(move_to);
            let push_mask = BitBoard::as_mask(push_to);
            board.oppo_worker_xor(other_god, !player, push_from | push_mask);
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        let mut result = self.move_mask();

        if let Some(push_pos) = self.push_to_position() {
            result |= BitBoard::as_mask(push_pos);
        }

        result
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_bool(self.push_to_position().is_some());
        helper.get()
    }
}

pub(super) fn minotaur_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(minotaur_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.mate_start_mask;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);
    let neighbor_map = prelude.standard_neighbor_map;

    let blocked_squares = prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        // Raw entries, not portal-swapped: Minotaur remaps entry->exit himself so he can also
        // push whoever stands on the entry whirlpool straight back.
        let mut worker_moves =
            get_basic_moves_from_raw_data_with_custom_blockers_no_portal::<MUST_CLIMB>(
                &prelude,
                worker_start_state.worker_start_pos,
                worker_start_state.worker_start_mask,
                worker_start_state.worker_start_height,
                worker_start_state.other_own_workers | prelude.domes_and_frozen,
            );

        let portal = prelude.portal_squares;
        let exit_blockers =
            (prelude.own_workers ^ worker_start_state.worker_start_mask) | prelude.oppo_workers;

        // Wins are scanned over the move's *outcome*: stepping onto a whirlpool lands the worker on
        // the other one, which wins at any height. With no portal this collapses to the old scan.
        if is_mate_only::<F>() || worker_start_state.can_mate || portal.is_not_empty() {
            let mut winning_entries = BitBoard::EMPTY;

            for entry_pos in worker_moves {
                let entry_mask = BitBoard::as_mask(entry_pos);
                let outcome_pos =
                    displacer_portal_exit(portal, exit_blockers, entry_pos).unwrap_or(entry_pos);

                if (BitBoard::as_mask(outcome_pos) & prelude.exactly_level_3 & prelude.win_mask)
                    .is_empty()
                {
                    continue;
                }

                let winning_move = if (entry_mask & prelude.oppo_workers).is_not_empty() {
                    // Pushing whoever holds the entry straight back; if they cannot be pushed
                    // (blocked/off board) Minotaur cannot step there, so no win here.
                    let Some(push_to) =
                        PUSH_MAPPING[worker_start_pos as usize][entry_pos as usize]
                    else {
                        continue;
                    };
                    if (BitBoard::as_mask(push_to) & blocked_squares).is_not_empty() {
                        continue;
                    }
                    MinotaurMove::new_minotaur_winning_push_move(
                        worker_start_pos,
                        outcome_pos,
                        push_to,
                    )
                } else {
                    MinotaurMove::new_winning_move(worker_start_pos, outcome_pos)
                };

                result.push(ScoredMove::new_winning_move(winning_move.into()));
                winning_entries |= entry_mask;
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }

            worker_moves ^= winning_entries;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for entry_pos in worker_moves {
            let entry_mask = BitBoard::as_mask(entry_pos);

            // The push is decided at the entry Minotaur steps onto; the whirlpool may then teleport
            // him past it to the exit, which is where he ends up and builds from.
            let mut worker_end_pos =
                displacer_portal_exit(portal, exit_blockers, entry_pos).unwrap_or(entry_pos);
            let mut worker_end_mask = BitBoard::as_mask(worker_end_pos);

            let mut push_to_spot: Option<Square> = None;
            let mut push_to_mask = BitBoard::EMPTY;

            let mut final_build_mask = prelude.build_mask;
            let mut other_workers_post_push = prelude.oppo_workers;

            if (entry_mask & prelude.oppo_workers).is_not_empty() {
                if let Some(push_to) =
                    PUSH_MAPPING[worker_start_state.worker_start_pos as usize][entry_pos as usize]
                {
                    let tmp_push_to_mask = BitBoard::as_mask(push_to);
                    if (tmp_push_to_mask & blocked_squares).is_empty() {
                        push_to_spot = Some(push_to);
                        push_to_mask = tmp_push_to_mask;

                        other_workers_post_push =
                            prelude.oppo_workers ^ push_to_mask ^ entry_mask;
                        final_build_mask =
                            prelude.other_god.get_build_mask(other_workers_post_push)
                                | prelude.exactly_level_3;
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            if prelude.is_against_harpies && push_to_spot.is_none() {
                // Harpies and Charybdis can never both be the opponent, so `portal` is empty and
                // `worker_end_pos` is just the entry - the slide behaves exactly as before.
                worker_end_pos = slide_position(
                    &prelude,
                    worker_start_state.worker_start_pos,
                    worker_end_pos,
                );
                worker_end_mask = BitBoard::as_mask(worker_end_pos);
            }

            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_state.worker_start_height;

            // Post-push occupancy, so Minotaur may build on a whirlpool he just cleared. (Without a
            // portal this equals the old `all_non_moving_workers` mask, since the pushed worker was
            // where he now stands and is excluded by the neighbour map anyway.)
            let mut worker_builds = NEIGHBOR_MAP[worker_end_pos as usize]
                & !(other_workers_post_push
                    | worker_start_state.other_own_workers
                    | prelude.domes_and_frozen);
            worker_builds &= final_build_mask;

            if is_interact_with_key_squares::<F>() {
                if ((worker_end_mask | push_to_mask) & key_squares).is_empty() {
                    worker_builds &= key_squares;
                }
            }

            let free_move_spaces = !(worker_start_state.other_own_workers
                | prelude.domes_and_frozen
                | worker_end_mask);
            let not_other_pushed_workers = !other_workers_post_push;

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);

                let new_action = if let Some(push_to) = push_to_spot {
                    MinotaurMove::new_minotaur_push_move(
                        worker_start_state.worker_start_pos,
                        worker_end_pos,
                        worker_build_pos,
                        push_to,
                    )
                } else {
                    MinotaurMove::new_minotaur_move(
                        worker_start_state.worker_start_pos,
                        worker_end_pos,
                        worker_build_pos,
                    )
                };

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask);
                    let possible_dest_board = final_level_3 & prelude.win_mask & free_move_spaces;
                    let checkable_own_workers = (worker_start_state.other_own_workers
                        | worker_end_mask)
                        & prelude.exactly_level_2;

                    let mut is_check = false;

                    if !prelude.is_against_hypnus || checkable_own_workers.count_ones() >= 2 {
                        let blocked_for_final_push_squares = worker_start_state.other_own_workers
                            | worker_end_mask
                            | prelude.domes_and_frozen
                            | (prelude.exactly_level_3 & worker_build_mask)
                            | other_workers_post_push;

                        for worker in checkable_own_workers {
                            let ns = neighbor_map[worker as usize] & possible_dest_board;
                            if (ns & not_other_pushed_workers).is_not_empty() {
                                is_check = true;
                                break;
                            } else {
                                for o in ns & other_workers_post_push {
                                    if let Some(push_to) = PUSH_MAPPING[worker as usize][o as usize]
                                    {
                                        let tmp_push_to_mask = BitBoard::as_mask(push_to);
                                        if (tmp_push_to_mask & blocked_for_final_push_squares)
                                            .is_empty()
                                        {
                                            is_check = true;
                                            break;
                                        }
                                    }
                                }
                                if is_check {
                                    break;
                                }
                            }
                        }
                    }

                    is_check
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

pub const fn build_minotaur() -> GodPower {
    god_power(
        GodName::Minotaur,
        build_god_power_movers!(minotaur_move_gen),
        build_god_power_actions::<MinotaurMove>(),
        16532879311019593353,
        196173323035994051,
    )
}

#[cfg(test)]
mod charybdis_portal_tests {
    use crate::board::{GameStateBuilder, GodData};
    use crate::gods::GodName;
    use crate::player::Player;
    use crate::square::Square::*;

    use super::*;

    fn with_whirlpools(mut state: FullGameState, player: Player, squares: &[Square]) -> FullGameState {
        let mut mask = BitBoard::EMPTY;
        for s in squares {
            mask |= BitBoard::as_mask(*s);
        }
        state.board.set_god_data(player, mask.0 as GodData);
        state
    }

    #[test]
    fn test_minotaur_pushes_off_a_whirlpool_and_teleports() {
        // Charybdis' worker sits on whirlpool D4. Minotaur steps onto D4 from C5, shoving her
        // straight back to E3, and is teleported to the free partner E1.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Minotaur)
                .with_p1_worker(D4)
                .with_p1_worker(A1)
                .with_p2_worker(C5)
                .with_p2_worker(E5)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[D4, E1],
        );

        let minotaur = GodName::Minotaur.to_power();
        let mut found = false;
        for scored in minotaur.get_all_moves(&state, Player::Two) {
            let m: MinotaurMove = scored.action.into();
            if m.get_is_winning() || m.move_from_position() != C5 || m.move_to_position() != E1 {
                continue;
            }
            found = true;
            let next = state.next_state(minotaur, GodName::Charybdis.to_power(), scored.action);
            assert!(next.board.workers[Player::Two as usize].contains_square(E1));
            assert!(next.board.workers[Player::One as usize].contains_square(E3));
            assert!(!next.board.workers[Player::One as usize].contains_square(D4));
        }
        assert!(found, "expected Minotaur to push off D4 and surface at E1");
    }

    #[test]
    fn test_minotaur_wins_by_teleporting_onto_a_level_three_whirlpool() {
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Minotaur)
                .with_p1_worker(A1)
                .with_p1_worker(B1)
                .with_p2_worker(C5)
                .with_p2_worker(E5)
                .with_height(E1, 3)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[D4, E1],
        );
        // D4 empty; Minotaur steps onto it from C5 and surfaces on the level-3 E1 to win.
        let wins = GodName::Minotaur.to_power().get_winning_moves(&state, Player::Two);
        let mut found = false;
        for scored in &wins {
            let m: MinotaurMove = scored.action.into();
            if m.move_from_position() == C5 && m.move_to_position() == E1 {
                found = true;
                let next = state.next_state(
                    GodName::Minotaur.to_power(),
                    GodName::Charybdis.to_power(),
                    scored.action,
                );
                assert_eq!(next.get_winner(), Some(Player::Two));
            }
        }
        assert!(found, "expected a Minotaur portal win onto E1");
    }
}
