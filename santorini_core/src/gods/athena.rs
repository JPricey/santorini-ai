use crate::{
    add_scored_move,
    bitboard::BitBoard,
    board::{BoardState, FullGameState, NEIGHBOR_MAP},
    build_building_masks, build_god_power_movers, build_parse_flags, build_push_winning_moves,
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
pub const ATHENA_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const ATHENA_MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
pub const ATHENA_BUILD_POSITION_OFFSET: usize = ATHENA_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const ATHENA_DID_IMPROVE_OFFSET: usize = ATHENA_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const ATHENA_DID_IMPROVE_CHANGE_OFFSET: usize = ATHENA_DID_IMPROVE_OFFSET + 1;

pub const ATHENA_DID_IMPROVE_MASK: MoveData = 1 << ATHENA_DID_IMPROVE_OFFSET;
pub const ATHENA_DID_IMPROVE_CHANGE_MASK: MoveData = 1 << ATHENA_DID_IMPROVE_CHANGE_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AthenaMove(pub MoveData);

impl Into<GenericMove> for AthenaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AthenaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AthenaMove {
    pub fn new_athena_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        did_climb: bool,
        did_climb_change: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATHENA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATHENA_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << ATHENA_BUILD_POSITION_OFFSET)
            | ((did_climb) as MoveData) << ATHENA_DID_IMPROVE_OFFSET
            | ((did_climb_change) as MoveData) << ATHENA_DID_IMPROVE_CHANGE_OFFSET;

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << ATHENA_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << ATHENA_MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> ATHENA_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }

    pub fn get_did_climb(&self) -> bool {
        (self.0 & ATHENA_DID_IMPROVE_MASK) != 0
    }

    pub fn get_did_climb_change(&self) -> bool {
        (self.0 & ATHENA_DID_IMPROVE_CHANGE_MASK) != 0
    }
}

impl std::fmt::Debug for AthenaMove {
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
        } else if self.get_did_climb() {
            write!(f, "{}>{}!^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for AthenaMove {
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

        let build_position = self.build_position();
        board.build_up(build_position);
        board.flip_worker_can_climb(!board.current_player, self.get_did_climb_change())
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

        res
    }
}

fn athena_move_gen<const F: MoveGenFlags>(
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
       own_workers:  own_workers,
       other_workers:  other_workers,
       result:  result,
       all_workers_mask:  all_workers_mask,
       is_mate_only:  is_mate_only,
       current_workers:  current_workers,
       checkable_worker_positions_mask:  checkable_worker_positions_mask,
    );
    let did_not_improve_last_turn = board.get_worker_can_climb(!player);

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let other_own_workers = own_workers ^ moving_worker_start_mask;
        let other_threatening_workers = other_own_workers & checkable_worker_positions_mask;
        let mut other_threatening_neighbors = BitBoard::EMPTY;
        for other_pos in other_threatening_workers {
            other_threatening_neighbors |= NEIGHBOR_MAP[other_pos as usize];
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);

        if is_mate_only || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & exactly_level_3 & win_mask;

            build_push_winning_moves!(
                moves_to_level_3,
                worker_moves,
                AthenaMove::new_winning_move,
                moving_worker_start_pos,
                result,
                is_stop_on_mate,
            );
        }

        if is_mate_only {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | domes);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);
            let is_improving = worker_end_height > worker_starting_height;
            let not_own_workers = !(other_own_workers | moving_worker_end_mask);

            build_building_masks!(
                worker_end_pos: moving_worker_end_pos,
                open_squares: buildable_squares,
                build_mask: build_mask,
                is_interact_with_key_squares: is_interact_with_key_squares,
                key_squares_expr: (!is_improving && (moving_worker_end_mask & key_squares).is_empty()),
                key_squares: key_squares,

                all_possible_builds: all_possible_builds,
                narrowed_builds: narrowed_builds,
                worker_plausible_next_moves: worker_plausible_next_moves,
            );

            let is_now_lvl_2 = (worker_end_height == 2) as usize;
            let reach_board = if is_against_hypnus
                && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2
            {
                BitBoard::EMPTY
            } else {
                (other_threatening_neighbors
                    | (worker_plausible_next_moves & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                    & win_mask
            };

            for worker_build_pos in narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = AthenaMove::new_athena_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                    is_improving,
                    is_improving == did_not_improve_last_turn,
                );

                let is_check = {
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);
                    let check_board =
                        reach_board & final_level_3 & buildable_squares & not_own_workers;
                    check_board.is_not_empty()
                };

                add_scored_move!(new_action, is_include_score, is_check, is_improving, result);
            }
        }
    }

    result
}

pub const fn build_athena() -> GodPower {
    god_power(
        GodName::Athena,
        build_god_power_movers!(athena_move_gen),
        build_god_power_actions::<AthenaMove>(),
        1867170053174999423,
        15381411414297507361,
    )
}
