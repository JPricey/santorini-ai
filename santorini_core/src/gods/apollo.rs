use crate::{
    add_scored_move,
    bitboard::{BitBoard, NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers, build_parse_flags,
    gods::{
        FullAction, GodName, GodPower, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
    },
    player::Player,
    square::Square,
    variable_prelude,
};

use super::PartialAction;

// from(5)|to(5)|build(5)|win(1)
pub const APOLLO_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const APOLLO_MOVE_TO_POSITION_OFFSET: usize = APOLLO_MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_BUILD_POSITION_OFFSET: usize = APOLLO_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_DID_SWAP_OFFSET: usize = APOLLO_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const APOLLO_DID_SWAP_MASK: MoveData = 1 << APOLLO_DID_SWAP_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ApolloMove(pub MoveData);

impl Into<GenericMove> for ApolloMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ApolloMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ApolloMove {
    pub fn new_apollo_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        did_swap: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << APOLLO_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << APOLLO_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << APOLLO_BUILD_POSITION_OFFSET)
            | ((did_swap as MoveData) << APOLLO_DID_SWAP_OFFSET);

        Self(data)
    }

    pub fn new_apollo_winning_move(
        move_from_position: Square,
        move_to_position: Square,
        did_swap: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << APOLLO_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << APOLLO_MOVE_TO_POSITION_OFFSET)
            | ((did_swap as MoveData) << APOLLO_DID_SWAP_OFFSET)
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
        Square::from((self.0 >> APOLLO_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn did_swap(self) -> bool {
        self.0 & APOLLO_DID_SWAP_MASK != 0
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ApolloMove {
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
        } else if self.did_swap() {
            write!(f, "{}<>{}^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for ApolloMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![PartialAction::SelectWorker(self.move_from_position())];

        if self.did_swap() {
            res.push(PartialAction::MoveWorkerWithSwap(self.move_to_position()));
        } else {
            res.push(PartialAction::MoveWorker(self.move_to_position()));
        }

        if !self.get_is_winning() {
            res.push(PartialAction::Build(self.build_position()));
        }

        return vec![res];
    }

    fn make_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.did_swap() {
            board.worker_xor(!board.current_player, worker_move_mask);
        }

        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
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
        res = res * 2 + self.did_swap() as usize;

        res
    }
}

fn apollo_move_gen<const F: MoveGenFlags>(
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

    variable_prelude!(
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
       acting_workers:  acting_workers,
       checkable_worker_positions_mask:  checkable_worker_positions_mask,
    );

    for moving_worker_start_pos in acting_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let other_own_workers = own_workers ^ moving_worker_start_mask;
        let other_threatening_workers = other_own_workers & checkable_worker_positions_mask;

        let unblocked_from_final_moves = !(domes & other_own_workers);

        let mut other_threatening_neighbors = BitBoard::EMPTY;
        for other_pos in other_threatening_workers {
            other_threatening_neighbors |= NEIGHBOR_MAP[other_pos as usize];
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | own_workers);

        if is_mate_only || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & exactly_level_3 & win_mask;
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let is_swap =
                    (BitBoard::as_mask(moving_worker_end_pos) & oppo_workers).is_not_empty();
                let winning_move = ScoredMove::new_winning_move(
                    ApolloMove::new_apollo_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        is_swap,
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

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let blocked_squares_minus_opponent_workers = non_selected_workers | domes;

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let not_own_workers = !(other_own_workers | moving_worker_end_mask);
            let worker_end_height = board.get_height(moving_worker_end_pos);
            let is_improving = worker_end_height > worker_starting_height;

            let mut final_other_workers = oppo_workers;
            let mut final_build_mask = build_mask;
            let is_swap = (BitBoard::as_mask(moving_worker_end_pos) & oppo_workers).is_not_empty();
            if is_swap {
                final_other_workers ^= moving_worker_end_mask | moving_worker_start_mask;
                final_build_mask = other_god.get_build_mask(final_other_workers) | exactly_level_3;
            }
            let buildable_squares = !(blocked_squares_minus_opponent_workers | final_other_workers);

            let end_neighbors = NEIGHBOR_MAP[moving_worker_end_pos as usize];
            let mut worker_builds = end_neighbors & buildable_squares & final_build_mask;

            if is_interact_with_key_squares {
                if ((moving_worker_start_mask & BitBoard::CONDITIONAL_MASK[is_swap as usize]
                    | moving_worker_end_mask)
                    & key_squares)
                    .is_empty()
                {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let is_now_lvl_2 = (worker_end_height == 2) as usize;
            let reach_board = if is_against_hypnus
                && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2
            {
                BitBoard::EMPTY
            } else {
                other_threatening_neighbors
                    | (end_neighbors & BitBoard::CONDITIONAL_MASK[is_now_lvl_2])
            };

            for worker_build_pos in worker_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = ApolloMove::new_apollo_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                    is_swap,
                );
                let is_check = {
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask) & not_own_workers;
                    let check_board =
                        reach_board & final_level_3 & win_mask & unblocked_from_final_moves;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }
        }
    }

    result
}

pub const fn build_apollo() -> GodPower {
    god_power(
        GodName::Apollo,
        build_god_power_movers!(apollo_move_gen),
        build_god_power_actions::<ApolloMove>(),
        3394957705078584374,
        7355591628209476781,
    )
}
