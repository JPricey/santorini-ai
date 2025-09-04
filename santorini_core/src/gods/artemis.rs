use crate::{
    add_scored_move,
    bitboard::{BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
    build_building_masks, build_god_power_movers, build_parse_flags,
    gods::{
        FullAction, GodName, GodPower, PartialAction, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::slide_position_with_custom_worker_blocker,
    },
    non_checking_variable_prelude,
    player::Player,
    square::Square,
};

// ArtemisMove is an exact copy of MortalMove, except with a different blocker board calculation to
// account for the longer moves
// from(5)|to(5)|build(5)|win(1)
pub const ARTEMIS_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const ARTEMIS_MOVE_TO_POSITION_OFFSET: usize =
    ARTEMIS_MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
pub const ARTEMIS_BUILD_POSITION_OFFSET: usize = ARTEMIS_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ArtemisMove(pub MoveData);

impl GodMove for ArtemisMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        if self.get_is_winning() {
            return vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position()),
            ]];
        }

        let build_position = self.build_position();
        vec![vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position()),
            PartialAction::Build(build_position),
        ]]
    }

    fn make_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(board.current_player);
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

        res
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
    pub fn new_artemis_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << ARTEMIS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ARTEMIS_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ARTEMIS_BUILD_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_artemis_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData)
            << ARTEMIS_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ARTEMIS_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> ARTEMIS_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
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

fn artemis_move_gen_vs_harpies<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    build_parse_flags!(
        is_mate_only,
        is_include_score,
        is_stop_on_mate,
        is_interact_with_key_squares
    );

    non_checking_variable_prelude!(
       state:  state,
       player:  player,
       board:  board,
       other_player:  other_player,
       current_player_idx:  current_player_idx,
       other_player_idx:  other_player_idx,
       other_god:  other_god,
       exactly_level_0:  exactly_level_0,
       exactly_level_1:  exactly_level_1,
       exactly_level_2:  exactly_level_2,
       exactly_level_3:  exactly_level_3,
       domes:  domes,
       win_mask:  _win_mask,
       build_mask: _build_mask,
       is_against_hypnus: is_against_hypnus,
       is_against_harpies: _is_against_harpies,
       own_workers:  own_workers,
       oppo_workers:  oppo_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
       acting_workers: acting_workers,
    );

    if is_mate_only {
        acting_workers &= board.at_least_level_1()
    }

    let mut null_build_blocker = BitBoard::MAIN_SECTION_MASK;

    for worker_start_pos in acting_workers.into_iter() {
        let worker_start_mask = BitBoard::as_mask(worker_start_pos);
        let worker_start_height = board.get_height(worker_start_pos);
        let other_own_workers = own_workers ^ worker_start_mask;

        let other_threatening_workers = other_own_workers & exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);
        let non_selected_workers = all_workers_mask ^ worker_start_mask;
        let open_squares = !(non_selected_workers | domes);

        let mut worker_1d_moves = NEIGHBOR_MAP[worker_start_pos as usize]
            & !board.height_map[3.min(worker_start_height + 1)]
            & open_squares;

        let one_move_wins = worker_1d_moves & exactly_level_3;
        for worker_mid_pos in one_move_wins {
            let winning_move = ScoredMove::new_winning_move(
                ArtemisMove::new_artemis_winning_move(worker_start_pos, worker_mid_pos).into(),
            );
            result.push(winning_move);
            if is_stop_on_mate {
                return result;
            }
        }

        let mut already_output = one_move_wins;
        worker_1d_moves ^= already_output;
        let mut worker_final_destinations = BitBoard::EMPTY;

        let not_worker_start_mask = !worker_start_mask;

        for init_worker_mid_pos in worker_1d_moves {
            let worker_mid_pos = slide_position_with_custom_worker_blocker(
                &board,
                worker_start_pos,
                init_worker_mid_pos,
                non_selected_workers,
            );
            worker_final_destinations |= BitBoard::as_mask(worker_mid_pos);
            let mid_height = board.get_height(worker_mid_pos);

            let mut next_moves = NEIGHBOR_MAP[worker_mid_pos as usize]
                & !board.height_map[3.min(mid_height + 1)]
                & open_squares
                & !already_output
                & not_worker_start_mask;

            let next_wins = next_moves & exactly_level_3;
            next_moves ^= next_wins;
            already_output |= next_wins;
            for win_pos in next_wins {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(worker_start_pos, win_pos).into(),
                );
                result.push(winning_move);
                if is_stop_on_mate {
                    return result;
                }
            }
            if is_mate_only {
                continue;
            }

            for worker_end_pos in next_moves {
                let slid_worker_end_pos = slide_position_with_custom_worker_blocker(
                    &board,
                    worker_mid_pos,
                    worker_end_pos,
                    non_selected_workers,
                );
                let worker_end_mask = BitBoard::as_mask(slid_worker_end_pos);
                worker_final_destinations |= worker_end_mask;
            }
        }

        if is_mate_only {
            continue;
        }

        for worker_end_pos in worker_final_destinations & !already_output {
            let worker_end_mask = BitBoard::as_mask(worker_end_pos);
            already_output ^= worker_end_mask;

            let worker_end_height = board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_height;
            let is_now_lvl_2 = (worker_end_height == 2) as usize;

            let possible_builds = NEIGHBOR_MAP[worker_end_pos as usize] & open_squares;

            let reach_board = (other_threatening_neighbors
                | (possible_builds & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                & open_squares;

            let mut narrowed_builds = possible_builds;
            if is_interact_with_key_squares {
                if (key_squares & worker_end_mask).is_empty() {
                    narrowed_builds &= key_squares;
                }
            }

            if is_stop_on_mate && worker_end_pos == worker_start_pos {
                narrowed_builds &= null_build_blocker;
                null_build_blocker ^= narrowed_builds;
            }

            for build_pos in narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(build_pos);
                let new_action =
                    ArtemisMove::new_artemis_move(worker_start_pos, worker_end_pos, build_pos);
                let is_check = {
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }
        }
    }

    return result;
}

fn artemis_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if state.gods[!player as usize].is_harpies() {
        return artemis_move_gen_vs_harpies::<F>(state, player, key_squares);
    }

    build_parse_flags!(
        is_mate_only,
        is_include_score,
        is_stop_on_mate,
        is_interact_with_key_squares
    );

    non_checking_variable_prelude!(
       state:  state,
       player:  player,
       board:  board,
       other_player:  other_player,
       current_player_idx:  current_player_idx,
       other_player_idx:  other_player_idx,
       other_god:  other_god,
       exactly_level_0:  exactly_level_0,
       exactly_level_1:  exactly_level_1,
       exactly_level_2:  exactly_level_2,
       exactly_level_3:  exactly_level_3,
       domes:  domes,
       win_mask:  win_mask,
       build_mask: build_mask,
       is_against_hypnus: is_against_hypnus,
       is_against_harpies: _is_against_harpies,
       own_workers:  own_workers,
       oppo_workers:  oppo_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
       acting_workers: acting_workers,
    );

    let not_other_workers = !oppo_workers;

    if is_mate_only {
        acting_workers &= board.at_least_level_1()
    }
    let can_worker_climb = board.get_worker_can_climb(player);

    for worker_start_pos in acting_workers.into_iter() {
        let worker_start_mask = BitBoard::as_mask(worker_start_pos);
        let worker_start_height = board.get_height(worker_start_pos);
        let other_own_workers = own_workers ^ worker_start_mask;
        let other_checkable_workers = other_own_workers & board.at_least_level_1();

        let other_checkable_touching =
            apply_mapping_to_mask(other_checkable_workers, &INCLUSIVE_NEIGHBOR_MAP);
        let mut valid_destinations = !(all_workers_mask | domes);

        let mut worker_1d_moves = (NEIGHBOR_MAP[worker_start_pos as usize]
            & !board.height_map[board.get_worker_climb_height(player, worker_start_height)]
            | worker_start_mask)
            & valid_destinations;

        if worker_start_height == 2 {
            let wining_moves = worker_1d_moves & exactly_level_3 & win_mask;
            worker_1d_moves ^= wining_moves;
            valid_destinations ^= wining_moves;

            for moving_worker_end_pos in wining_moves.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(worker_start_pos, moving_worker_end_pos)
                        .into(),
                );
                result.push(winning_move);
                if is_stop_on_mate {
                    return result;
                }
            }
        }

        if can_worker_climb {
            let at_height_2_1d = worker_1d_moves & exactly_level_2;
            let mut winning_moves_to_level_3 = BitBoard::EMPTY;
            for pos in at_height_2_1d {
                winning_moves_to_level_3 |= NEIGHBOR_MAP[pos as usize];
            }
            winning_moves_to_level_3 &= exactly_level_3 & valid_destinations & win_mask;
            valid_destinations ^= winning_moves_to_level_3;

            for moving_worker_end_pos in winning_moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(worker_start_pos, moving_worker_end_pos)
                        .into(),
                );
                result.push(winning_move);
                if is_stop_on_mate {
                    return result;
                }
            }
        }

        if is_mate_only {
            continue;
        }

        let mut worker_moves = worker_1d_moves;
        let h_delta = can_worker_climb as usize;
        for h in [0, 1, 2, 3] {
            let current_level_workers = worker_1d_moves & !board.height_map[h];
            worker_1d_moves ^= current_level_workers;
            let current_level_destinations = !board.height_map[3.min(h + h_delta)];

            for end_pos in current_level_workers {
                worker_moves |= current_level_destinations & NEIGHBOR_MAP[end_pos as usize];
            }
        }
        worker_moves &= valid_destinations;

        let non_selected_workers = all_workers_mask ^ worker_start_mask;
        let buildable_squares = !(non_selected_workers | domes);
        for worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(worker_end_pos);
            let worker_end_height = board.get_height(worker_end_pos);
            let is_improving = worker_end_height > worker_start_height;
            let not_any_workers = !(oppo_workers | other_own_workers | moving_worker_end_mask);

            build_building_masks!(
                worker_end_pos: worker_end_pos,
                open_squares: buildable_squares,
                build_mask: build_mask,
                is_interact_with_key_squares: is_interact_with_key_squares,
                key_squares_expr: (moving_worker_end_mask & key_squares).is_empty(),
                key_squares: key_squares,

                all_possible_builds: all_possible_builds,
                narrowed_builds: narrowed_builds,
                worker_plausible_next_moves: _worker_plausible_next_moves,
            );

            let mut own_touching = BitBoard::EMPTY;
            if worker_end_height >= 1 {
                own_touching |= INCLUSIVE_NEIGHBOR_MAP[worker_end_pos as usize];
            }

            let mut final_touching = other_checkable_touching | own_touching;
            if is_against_hypnus {
                // Against hypnus, pretend you can't get to lvl 3
                let has_other_lvl_2 = (other_checkable_workers & exactly_level_2).is_not_empty();
                let has_other_lvl_1 = (other_checkable_workers & exactly_level_1).is_not_empty();

                if (has_other_lvl_2 && worker_end_height == 2)
                    || (has_other_lvl_1 && worker_end_height == 1)
                {
                    // Good
                } else if has_other_lvl_2 && worker_end_height == 1 {
                    final_touching = own_touching & !other_own_workers;
                } else if has_other_lvl_1 && worker_end_height == 2 {
                    final_touching = other_checkable_touching & !moving_worker_end_mask;
                } else {
                    final_touching = BitBoard::EMPTY;
                }
            }

            for worker_build_pos in narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = ArtemisMove::new_artemis_move(
                    worker_start_pos,
                    worker_end_pos,
                    worker_build_pos,
                );

                let final_l3 = ((exactly_level_3 & !worker_build_mask)
                    | (exactly_level_2 & worker_build_mask))
                    & not_any_workers
                    & win_mask;

                let is_check = final_touching.is_not_empty() && final_l3.is_not_empty() && {
                    let final_l2 = ((exactly_level_2 & !worker_build_mask)
                        | (exactly_level_1 & worker_build_mask))
                        & not_other_workers;

                    let final_touching_checks = apply_mapping_to_mask(final_l3, &NEIGHBOR_MAP);
                    (final_touching & final_touching_checks & final_l2).is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
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
