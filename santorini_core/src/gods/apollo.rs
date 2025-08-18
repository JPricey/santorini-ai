use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    build_god_power,
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            GenericMove, GodMove, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES,
            LOWER_POSITION_MASK, MATE_ONLY, MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags,
            NULL_MOVE_DATA, POSITION_WIDTH, STOP_ON_MATE, ScoredMove,
        },
    },
    player::Player,
    square::Square,
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

    fn unmake_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.did_swap() {
            board.worker_xor(!board.current_player, worker_move_mask);
        }

        if self.get_is_winning() {
            board.unset_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.unbuild(build_position);
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
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let base_current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let mut current_workers = base_current_workers;
    let opponent_workers = board.workers[1 - current_player_idx];

    let all_workers_mask = current_workers | opponent_workers;

    if F & MATE_ONLY != 0 {
        current_workers &= board.exactly_level_2()
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);
        let other_own_workers = current_workers ^ moving_worker_start_mask;
        let mut neighbor_check_if_builds = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_lvl_2 = other_own_workers & board.exactly_level_2();
            for other_pos in other_lvl_2 {
                neighbor_check_if_builds |=
                    NEIGHBOR_MAP[other_pos as usize] & board.exactly_level_2();
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | base_current_workers);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let is_swap =
                    (BitBoard::as_mask(moving_worker_end_pos) & opponent_workers).is_not_empty();
                let winning_move = ScoredMove::new_winning_move(
                    ApolloMove::new_apollo_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                        is_swap,
                    )
                    .into(),
                );
                result.push(winning_move);
                if F & STOP_ON_MATE != 0 {
                    return result;
                }
            }
        }

        if F & MATE_ONLY != 0 {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let non_swapped_buildable_squares = !(non_selected_workers | board.height_map[3]);

        let swapped_buildable_squares = !(all_workers_mask | board.height_map[3]);

        let worker_builds_by_is_swap = [non_swapped_buildable_squares, swapped_buildable_squares];
        for moving_worker_end_pos in worker_moves.into_iter() {
            let is_swap =
                (BitBoard::as_mask(moving_worker_end_pos) & opponent_workers).is_not_empty();
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);

            let mut worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize]
                & worker_builds_by_is_swap[is_swap as usize];

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let mut check_if_builds = neighbor_check_if_builds;
            let mut anti_check_builds = BitBoard::EMPTY;
            let mut is_already_check = false;

            if F & INCLUDE_SCORE != 0 {
                if worker_end_height == 2 {
                    check_if_builds |= worker_builds & board.exactly_level_2();
                    anti_check_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize]
                        & board.exactly_level_3()
                        & !other_own_workers;
                    is_already_check = anti_check_builds != BitBoard::EMPTY;
                }
            }

            for worker_build_pos in worker_builds {
                let new_action = ApolloMove::new_apollo_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                    is_swap,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    if is_already_check && (anti_check_builds & !worker_build_mask).is_not_empty()
                        || (worker_build_mask & check_if_builds).is_not_empty()
                    {
                        result.push(ScoredMove::new_checking_move(new_action.into()));
                    } else {
                        let is_improving = worker_end_height > worker_starting_height;
                        if is_improving {
                            result.push(ScoredMove::new_improving_move(new_action.into()));
                        } else {
                            result.push(ScoredMove::new_non_improver(new_action.into()));
                        };
                    }
                } else {
                    result.push(ScoredMove::new_unscored_move(new_action.into()));
                }
            }
        }
    }

    result
}

build_god_power!(
    build_apollo,
    god_name: GodName::Apollo,
    move_type: ApolloMove,
    move_gen: apollo_move_gen,
    hash1: 3394957705078584374,
    hash2: 7355591628209476781,
);
