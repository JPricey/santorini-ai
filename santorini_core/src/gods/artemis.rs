use crate::{
    bitboard::{BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, PartialAction, StaticGod,
        build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::slide_position_with_custom_worker_blocker,
        hypnus::hypnus_moveable_worker_filter,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_inclusive_movement_neighbors,
            get_sized_result, get_wind_reverse_neighbor_map, get_worker_climb_height_raw,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            is_stop_on_mate,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

// ArtemisMove is an exact copy of MortalMove, except with a different blocker board calculation to
// account for the longer moves
// from(5)|to(5)|build(5)|win(1)
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ArtemisMove(pub MoveData);

impl GodMove for ArtemisMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, board: &BoardState) -> BitBoard {
        let from = self.move_from_position();
        let to = self.move_to_position();

        (NEIGHBOR_MAP[from as usize] & NEIGHBOR_MAP[to as usize]) & board.exactly_level_2()
            | BitBoard::as_mask(self.move_to_position())
            | BitBoard::as_mask(self.move_from_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for ArtemisMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ArtemisMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ArtemisMove {
    fn new_artemis_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_artemis_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
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

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ArtemisMove {
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
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

fn artemis_move_gen_vs_harpies<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let checkable_worker_positions =
        prelude.exactly_level_1 | prelude.exactly_level_2 | prelude.exactly_level_3;
    let mut acting_workers = prelude.own_workers;
    if prelude.is_against_hypnus {
        acting_workers = hypnus_moveable_worker_filter(prelude.board, acting_workers);
    }
    if is_mate_only::<F>() {
        acting_workers &= checkable_worker_positions;
    }

    let mut null_build_blocker = BitBoard::MAIN_SECTION_MASK;

    for worker_start_pos in acting_workers.into_iter() {
        let worker_start_mask = BitBoard::as_mask(worker_start_pos);
        let worker_start_height = prelude.board.get_height(worker_start_pos);
        let other_own_workers = prelude.own_workers ^ worker_start_mask;

        let other_threatening_workers = other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);
        let non_selected_workers = prelude.all_workers_and_frozen_mask ^ worker_start_mask;
        let open_squares = !(non_selected_workers | prelude.domes_and_frozen);

        let mut worker_1d_moves = NEIGHBOR_MAP[worker_start_pos as usize]
            & !prelude.board.height_map[3.min(worker_start_height + 1)]
            & open_squares;

        let one_move_wins = worker_1d_moves & prelude.exactly_level_3;
        for worker_mid_pos in one_move_wins {
            let winning_move = ScoredMove::new_winning_move(
                ArtemisMove::new_artemis_winning_move(worker_start_pos, worker_mid_pos).into(),
            );
            result.push(winning_move);
            if is_stop_on_mate::<F>() {
                return result;
            }
        }

        let mut already_output = one_move_wins;
        worker_1d_moves ^= already_output;
        let mut worker_final_destinations = BitBoard::EMPTY;

        let not_worker_start_mask = !worker_start_mask;

        for init_worker_mid_pos in worker_1d_moves {
            let worker_mid_pos = slide_position_with_custom_worker_blocker(
                &prelude.board,
                worker_start_pos,
                init_worker_mid_pos,
                non_selected_workers,
            );
            worker_final_destinations |= BitBoard::as_mask(worker_mid_pos);
            let mid_height = prelude.board.get_height(worker_mid_pos);

            let mut next_moves = NEIGHBOR_MAP[worker_mid_pos as usize]
                & !prelude.board.height_map[3.min(mid_height + 1)]
                & open_squares
                & !already_output
                & not_worker_start_mask;

            let next_wins = next_moves & prelude.exactly_level_3;
            next_moves ^= next_wins;
            already_output |= next_wins;
            for win_pos in next_wins {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(worker_start_pos, win_pos).into(),
                );
                result.push(winning_move);
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }
            if is_mate_only::<F>() {
                continue;
            }

            for worker_end_pos in next_moves {
                let slid_worker_end_pos = slide_position_with_custom_worker_blocker(
                    &prelude.board,
                    worker_mid_pos,
                    worker_end_pos,
                    non_selected_workers,
                );
                let worker_end_mask = BitBoard::as_mask(slid_worker_end_pos);
                worker_final_destinations |= worker_end_mask;
            }
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_final_destinations & !already_output {
            let worker_end_mask = BitBoard::as_mask(worker_end_pos);
            already_output ^= worker_end_mask;

            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_height;
            let is_now_lvl_2 = (worker_end_height == 2) as usize;

            let possible_builds = NEIGHBOR_MAP[worker_end_pos as usize] & open_squares;

            let reach_board = (other_threatening_neighbors
                | (possible_builds & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                & open_squares;

            let mut narrowed_builds = possible_builds;
            if is_interact_with_key_squares::<F>() {
                if (key_squares & worker_end_mask).is_empty() {
                    narrowed_builds &= key_squares;
                }
            }

            if is_stop_on_mate::<F>() && worker_end_pos == worker_start_pos {
                narrowed_builds &= null_build_blocker;
                null_build_blocker ^= narrowed_builds;
            }

            for worker_build_pos in narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                    | (prelude.exactly_level_3 & !worker_build_mask);
                let check_board = reach_board & final_level_3;
                let is_check = check_board.is_not_empty();

                let new_action = ArtemisMove::new_artemis_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    is_improving,
                ));
            }
        }
    }

    return result;
}

fn artemis_vs_persephone<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let checkable_worker_positions = prelude.board.at_least_level_1();
    let mut acting_workers = prelude.own_workers;

    if is_mate_only::<F>() {
        acting_workers &= checkable_worker_positions;
    }
    let not_other_workers = !prelude.oppo_workers;
    let open_squares_for_move = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);

    for worker_start_pos in acting_workers.into_iter() {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let open_squares_for_build =
            !(worker_start_state.all_non_moving_workers | prelude.domes_and_frozen);
        let other_checkable_workers =
            worker_start_state.other_own_workers & checkable_worker_positions;
        let other_checkable_touching =
            apply_mapping_to_mask(other_checkable_workers, &INCLUSIVE_NEIGHBOR_MAP);

        let all_worker_1d_moves = NEIGHBOR_MAP[worker_start_pos as usize]
            & open_squares_for_move
            & !state.board.height_map[3.min(worker_start_state.worker_start_height + 1)];

        let mut worker_1d_improvers =
            all_worker_1d_moves & state.board.height_map[worker_start_state.worker_start_height];
        let worker_1d_non_improvers = all_worker_1d_moves ^ worker_1d_improvers;

        if worker_start_state.worker_start_height == 2 {
            for moving_worker_end_pos in worker_1d_improvers.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(worker_start_pos, moving_worker_end_pos)
                        .into(),
                );
                result.push(winning_move);
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }
            worker_1d_improvers = BitBoard::EMPTY;
        }

        let improvers_dest_mask =
            !state.board.height_map[3.min(worker_start_state.worker_start_height + 2)];
        let mut final_squares = worker_1d_improvers;
        for pos in worker_1d_improvers {
            final_squares |= NEIGHBOR_MAP[pos as usize];
        }
        final_squares &= improvers_dest_mask;

        final_squares |= apply_mapping_to_mask(
            worker_1d_non_improvers & prelude.exactly_level_2,
            &NEIGHBOR_MAP,
        ) & prelude.exactly_level_3;

        final_squares |= apply_mapping_to_mask(
            worker_1d_non_improvers & prelude.exactly_level_0,
            &NEIGHBOR_MAP,
        ) & prelude.exactly_level_1;
        final_squares |= apply_mapping_to_mask(
            worker_1d_non_improvers & prelude.exactly_level_1,
            &NEIGHBOR_MAP,
        ) & prelude.exactly_level_2;

        final_squares &= open_squares_for_move;

        let winners = final_squares & prelude.exactly_level_3;
        for moving_worker_end_pos in winners.into_iter() {
            let winning_move = ScoredMove::new_winning_move(
                ArtemisMove::new_artemis_winning_move(worker_start_pos, moving_worker_end_pos)
                    .into(),
            );
            result.push(winning_move);
            if is_stop_on_mate::<F>() {
                return result;
            }
        }

        if is_mate_only::<F>() {
            continue;
        }
        final_squares ^= winners;

        for worker_end_pos in final_squares.into_iter() {
            let worker_end_mask = BitBoard::as_mask(worker_end_pos);
            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_state.worker_start_height;

            let all_possible_builds =
                NEIGHBOR_MAP[worker_end_pos as usize] & open_squares_for_build;
            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let is_already_matched =
                    (worker_end_mask & prelude.key_squares).is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            let mut own_touching = BitBoard::EMPTY;
            if worker_end_height >= 1 {
                own_touching |= INCLUSIVE_NEIGHBOR_MAP[worker_end_pos as usize];
            }

            let final_touching = other_checkable_touching | own_touching;

            for worker_build_pos in narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let final_l3 = ((prelude.exactly_level_3 & !worker_build_mask)
                    | (prelude.exactly_level_2 & worker_build_mask))
                    & not_other_workers
                    & prelude.win_mask;

                let is_check = final_touching.is_not_empty() && final_l3.is_not_empty() && {
                    let final_l2 = ((prelude.exactly_level_2 & !worker_build_mask)
                        | (prelude.exactly_level_1 & worker_build_mask))
                        & not_other_workers;

                    let final_touching_checks = apply_mapping_to_mask(final_l3, &NEIGHBOR_MAP);
                    (final_touching & final_touching_checks & final_l2).is_not_empty()
                };

                let new_action = ArtemisMove::new_artemis_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    is_improving,
                ));
            }
        }
    }

    result
}

fn artemis_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if MUST_CLIMB {
        return artemis_vs_persephone::<F>(state, player, key_squares);
    }

    if state.gods[!player as usize].is_harpies() {
        return artemis_move_gen_vs_harpies::<F, MUST_CLIMB>(state, player, key_squares);
    }

    let mut result = persephone_check_result!(artemis_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let checkable_worker_positions = prelude.board.at_least_level_1();
    let mut acting_workers = prelude.own_workers;
    if prelude.is_against_hypnus {
        acting_workers = hypnus_moveable_worker_filter(prelude.board, acting_workers);
    }
    if is_mate_only::<F>() {
        acting_workers &= checkable_worker_positions;
    }

    let not_other_workers = !prelude.oppo_workers;

    let neighbor_map_ref = prelude.standard_neighbor_map;
    let inclusive_neighbor_map = get_inclusive_movement_neighbors(&prelude);
    let reverse_neighbor_map = get_wind_reverse_neighbor_map(&prelude);

    for worker_start_pos in acting_workers.into_iter() {
        let worker_start_mask = BitBoard::as_mask(worker_start_pos);
        let worker_start_height = prelude.board.get_height(worker_start_pos);

        let other_own_workers = prelude.own_workers ^ worker_start_mask;
        let other_checkable_workers = other_own_workers & checkable_worker_positions;

        let other_checkable_touching =
            apply_mapping_to_mask(other_checkable_workers, &inclusive_neighbor_map);
        let valid_half_destinations =
            !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);
        let mut valid_final_destinations =
            if (worker_start_mask & prelude.affinity_area).is_not_empty() {
                valid_half_destinations & prelude.affinity_area
            } else {
                valid_half_destinations
            };

        let down_mask = if prelude.is_down_prevented && worker_start_height > 0 {
            !prelude.board.height_map[worker_start_height - 1]
        } else {
            BitBoard::EMPTY
        };
        let mut worker_1d_moves = (neighbor_map_ref[worker_start_pos as usize]
            & !(prelude.board.height_map
                [get_worker_climb_height_raw(worker_start_height, prelude.can_climb)]
                | down_mask)
            | worker_start_mask)
            & valid_half_destinations;

        if worker_start_height == 2 {
            let wining_moves = worker_1d_moves
                & prelude.exactly_level_3
                & prelude.win_mask
                & valid_final_destinations;
            worker_1d_moves ^= wining_moves;
            valid_final_destinations ^= wining_moves;

            for moving_worker_end_pos in wining_moves.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(worker_start_pos, moving_worker_end_pos)
                        .into(),
                );
                result.push(winning_move);
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }
        }

        if prelude.can_climb {
            let at_height_2_1d = worker_1d_moves & prelude.exactly_level_2;
            let mut winning_moves_to_level_3 =
                apply_mapping_to_mask(at_height_2_1d, &neighbor_map_ref);
            winning_moves_to_level_3 &=
                prelude.exactly_level_3 & valid_final_destinations & prelude.win_mask;
            valid_final_destinations ^= winning_moves_to_level_3;

            for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(worker_start_pos, moving_worker_end_pos)
                        .into(),
                );
                result.push(winning_move);
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }
        }

        if is_mate_only::<F>() {
            continue;
        }

        let mut worker_moves = worker_1d_moves;
        let h_delta = prelude.can_climb as usize;

        if prelude.is_down_prevented {
            worker_moves |=
                apply_mapping_to_mask(worker_1d_moves & prelude.exactly_level_0, &neighbor_map_ref)
                    & (prelude.exactly_level_0 | prelude.exactly_level_1);
            worker_moves |=
                apply_mapping_to_mask(worker_1d_moves & prelude.exactly_level_1, &neighbor_map_ref)
                    & (prelude.exactly_level_1 | prelude.exactly_level_2);
            // Don't need to count level 2->3, since we already checked for wins
            worker_moves |=
                apply_mapping_to_mask(worker_1d_moves & prelude.exactly_level_2, &neighbor_map_ref)
                    & (prelude.exactly_level_2);
            // Don't need to check for moves from lvl 3, since we are against hades and that can't
            // happen
        } else {
            for h in [0, 1, 2, 3] {
                let current_level_workers = worker_1d_moves & !prelude.board.height_map[h];
                worker_1d_moves ^= current_level_workers;
                let current_level_destinations = !prelude.board.height_map[3.min(h + h_delta)];

                for end_pos in current_level_workers {
                    worker_moves |= current_level_destinations & neighbor_map_ref[end_pos as usize];
                }
            }
        }

        worker_moves &= valid_final_destinations;

        let non_selected_workers = prelude.all_workers_and_frozen_mask ^ worker_start_mask;
        let buildable_squares = !(non_selected_workers | prelude.domes_and_frozen);
        for worker_end_pos in worker_moves.into_iter() {
            let worker_end_mask = BitBoard::as_mask(worker_end_pos);
            let worker_end_height = prelude.board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_height;
            let not_any_workers = !(prelude.oppo_workers | other_own_workers | worker_end_mask);

            let all_possible_builds =
                NEIGHBOR_MAP[worker_end_pos as usize] & buildable_squares & prelude.build_mask;
            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let is_already_matched =
                    (worker_end_mask & prelude.key_squares).is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            let mut own_touching = BitBoard::EMPTY;
            if worker_end_height >= 1 {
                own_touching |= inclusive_neighbor_map[worker_end_pos as usize];
            }

            let mut final_touching =
                (other_checkable_touching | own_touching) & !prelude.domes_and_frozen;
            if prelude.is_against_hypnus {
                // Against hypnus, don't worry about other workers being on level 3 already
                let has_other_lvl_2 =
                    (other_checkable_workers & prelude.exactly_level_2).is_not_empty();
                let has_other_lvl_1 =
                    (other_checkable_workers & prelude.exactly_level_1).is_not_empty();

                if (has_other_lvl_2 && worker_end_height == 2)
                    || (has_other_lvl_1 && worker_end_height == 1)
                {
                    // Good
                } else if has_other_lvl_2 && worker_end_height == 1 {
                    final_touching = own_touching & !other_own_workers;
                } else if has_other_lvl_1 && worker_end_height == 2 {
                    final_touching = other_checkable_touching & !worker_end_mask;
                } else {
                    final_touching = BitBoard::EMPTY;
                }
            }

            for worker_build_pos in narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let final_l3 = ((prelude.exactly_level_3
                    & !(worker_build_mask | prelude.domes_and_frozen))
                    | (prelude.exactly_level_2 & worker_build_mask))
                    & not_any_workers
                    & prelude.win_mask;

                let is_check = final_touching.is_not_empty() && final_l3.is_not_empty() && {
                    let final_l2 = ((prelude.exactly_level_2 & !worker_build_mask)
                        | (prelude.exactly_level_1 & worker_build_mask))
                        & not_other_workers;

                    let final_touching_checks =
                        apply_mapping_to_mask(final_l3, &reverse_neighbor_map);
                    (final_touching & final_touching_checks & final_l2).is_not_empty()
                };

                let new_action = ArtemisMove::new_artemis_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    is_improving,
                ));
            }
        }
    }

    result
}

pub const fn build_artemis() -> GodPower {
    god_power(
        GodName::Artemis,
        build_god_power_movers!(artemis_move_gen),
        build_god_power_actions::<ArtemisMove>(),
        12504034891281202406,
        10874494938488172730,
    )
}
#[cfg(test)]
mod tests {
    use crate::{board::FullGameState, gods::GodName, player::Player};

    #[test]
    fn test_artemis_basic() {
        let state =
            FullGameState::try_from("0000022222000000000000000/1/artemis:0,1/artemis:23,24")
                .unwrap();

        let next_states = state.get_next_states_interactive();
        assert_eq!(next_states.len(), 10);
    }

    #[test]
    fn test_artemis_cant_move_through_wins() {
        let state =
            FullGameState::try_from("2300044444000000000000000/1/artemis:0/artemis:24").unwrap();
        let next_states = state.get_next_states_interactive();
        assert_eq!(next_states.len(), 1);
        assert_eq!(next_states[0].state.board.get_winner(), Some(Player::One))
    }

    #[test]
    fn test_artemis_win_check() {
        let artemis = GodName::Artemis.to_power();
        // Regular 1>2>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from(
                        "12300 44444 44444 44444 00000/1/artemis:0/artemis:24"
                    )
                    .unwrap(),
                    Player::One
                )
                .len(),
            1
        );

        // Can't move 1>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from(
                        "13300 44444 44444 44444 00000/1/artemis:0/artemis:24"
                    )
                    .unwrap(),
                    Player::One
                )
                .len(),
            0
        );

        // Can move 2>2>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from(
                        "22300 44444 44444 44444 00000/1/artemis:0/artemis:24"
                    )
                    .unwrap(),
                    Player::One
                )
                .len(),
            1
        );

        // Can't move 2>1>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from(
                        "21300 44444 44444 44444 00000/1/artemis:0/artemis:24"
                    )
                    .unwrap(),
                    Player::One
                )
                .len(),
            0
        );

        // Single move 2>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from(
                        "23000 44444 44444 44444 00000/1/artemis:0/artemis:24"
                    )
                    .unwrap(),
                    Player::One
                )
                .len(),
            1
        );

        // Can't win from 3>3
        assert_eq!(
            artemis
                .get_winning_moves(
                    &FullGameState::try_from(
                        "33000 44444 44444 44444 00000/1/artemis:0/artemis:24"
                    )
                    .unwrap(),
                    Player::One
                )
                .len(),
            0
        );
    }

    #[test]
    fn test_artemis_vs_harpies() {
        // let state =
        //     FullGameState::try_from("0000000000000000000000000/1/artemis:A5/harpies:E1").unwrap();

        // let next_states = state.get_next_states_interactive();
        // for state in &next_states {
        //     eprintln!("{:?} {:?}", state.state, state.actions);
        //     state.state.print_to_console();
        // }
    }
}
