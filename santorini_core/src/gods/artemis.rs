use crate::{
    add_scored_move,
    bitboard::{BitBoard, NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_building_masks, build_god_power_movers, build_parse_flags,
    gods::{
        build_god_power_actions, generic::{
            GenericMove, GodMove, MoveData, MoveGenFlags, ScoredMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, NULL_MOVE_DATA, POSITION_WIDTH
        }, god_power, hypnus::hypnus_moveable_worker_filter, FullAction, GodName, GodPower, PartialAction
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
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
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

fn artemis_move_gen<const F: MoveGenFlags>(
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
       win_mask:  win_mask,
       build_mask: build_mask,
       own_workers:  own_workers,
       other_workers:  other_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
    );

    let is_against_hypnus = other_god.is_hypnus();
    let not_other_workers = !other_workers;

    let mut current_workers = own_workers;
    if is_against_hypnus {
        current_workers = hypnus_moveable_worker_filter(board, current_workers);
    }
    if is_mate_only {
        current_workers &= board.at_least_level_1()
    }
    let can_worker_climb = board.get_worker_can_climb(player);

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);
        let other_own_workers = own_workers ^ moving_worker_start_mask;
        let other_checkable_workers = other_own_workers & board.at_least_level_1();

        let mut other_checkable_touching = BitBoard::EMPTY;
        for o in other_checkable_workers {
            other_checkable_touching |= NEIGHBOR_MAP[o as usize];
            other_checkable_touching |= BitBoard::as_mask(o);
        }

        let mut valid_destinations = !(all_workers_mask | domes);

        let mut worker_1d_moves = (NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
            | moving_worker_start_mask)
            & valid_destinations;

        if worker_starting_height == 2 {
            let wining_moves = worker_1d_moves & exactly_level_3 & win_mask;
            worker_1d_moves ^= wining_moves;
            valid_destinations ^= wining_moves;

            for moving_worker_end_pos in wining_moves.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    ArtemisMove::new_artemis_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                    )
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
                    ArtemisMove::new_artemis_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                    )
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

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | domes);
        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);
            let is_improving = worker_end_height > worker_starting_height;
            let not_any_workers = !(other_workers | other_own_workers | moving_worker_end_mask);

            build_building_masks!(
                worker_end_pos: moving_worker_end_pos,
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
                own_touching |= NEIGHBOR_MAP[moving_worker_end_pos as usize];
                own_touching |= moving_worker_end_mask;
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
                let build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = ArtemisMove::new_artemis_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );

                let final_l3 = ((exactly_level_3 & !build_mask) | (exactly_level_2 & build_mask))
                    & not_any_workers
                    & win_mask;

                let is_check = final_touching.is_not_empty() && final_l3.is_not_empty() && {
                    let final_l2 = ((exactly_level_2 & !build_mask)
                        | (exactly_level_1 & build_mask))
                        & not_other_workers;

                    let mut final_touching_checks = BitBoard::EMPTY;
                    for s in final_l3 {
                        final_touching_checks |= NEIGHBOR_MAP[s as usize];
                    }

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
}
