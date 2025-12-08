use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
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
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const IS_STONE_BIT_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

const IS_STONE_BIT_MASK: MoveData = 1 << IS_STONE_BIT_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MedusaMove(pub MoveData);

impl GodMove for MedusaMove {
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

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());

        if self.is_stone_end_of_turn() {
            let oppo_workers = board.workers[!player as usize];
            let mut frozen_squares = BitBoard::EMPTY;
            if other_god.god_name == GodName::Clio {
                frozen_squares = other_god.get_frozen_mask(board, !player);
            }

            let mut all_stones = BitBoard::EMPTY;
            for own_worker_pos in board.workers[player as usize] {
                let own_worker_height = board.get_height(own_worker_pos);
                if own_worker_height == 0 {
                    continue;
                }
                all_stones |= NEIGHBOR_MAP[own_worker_pos as usize]
                    & oppo_workers
                    & !(board.height_map[own_worker_height - 1] | frozen_squares);
            }

            board.oppo_worker_kill(other_god, !player, all_stones);
            for stone_pos in all_stones {
                board.build_up(stone_pos)
            }

            debug_assert!(
                all_stones.is_not_empty(),
                "Medusa claimed to stone someone, but could not resolve"
            );
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_bool(self.is_stone_end_of_turn());
        helper.get()
    }
}

impl Into<GenericMove> for MedusaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for MedusaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl MedusaMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        is_stone: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((is_stone as MoveData) << IS_STONE_BIT_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
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
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn is_stone_end_of_turn(self) -> bool {
        (self.0 & IS_STONE_BIT_MASK) != 0
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for MedusaMove {
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
        } else if self.is_stone_end_of_turn() {
            write!(f, "{}>{}^{} (S)", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

pub(super) fn medusa_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(medusa_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, MedusaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                MedusaMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let mut neighbor_stone_map = BitBoard::EMPTY;

        for other_worker in worker_start_state.other_own_workers {
            let worker_height = prelude.board.get_height(other_worker);
            if worker_height == 0 {
                continue;
            }
            neighbor_stone_map |= NEIGHBOR_MAP[other_worker as usize]
                & prelude.oppo_workers
                & !(prelude.board.height_map[worker_height - 1] | prelude.domes_and_frozen);
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let final_stone_map = if worker_end_move_state.worker_end_height > 0 {
                let current_stone_map = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                    & prelude.oppo_workers
                    & !(prelude.board.height_map[worker_end_move_state.worker_end_height - 1]
                        | prelude.domes_and_frozen);
                neighbor_stone_map | current_stone_map
            } else {
                neighbor_stone_map
            };

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);
            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;
            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let is_already_matched = ((final_stone_map | worker_end_move_state.worker_end_mask)
                    & prelude.key_squares)
                    .is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares | final_stone_map,
            );

            for worker_build_pos in narrowed_builds {
                let new_action = MedusaMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                    final_stone_map.is_not_empty(),
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & (build_mask | final_stone_map))
                        | (prelude.exactly_level_3 & !build_mask);
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

pub const fn build_medusa() -> GodPower {
    god_power(
        GodName::Medusa,
        build_god_power_movers!(medusa_move_gen),
        build_god_power_actions::<MedusaMove>(),
        8549903969002325999,
        1897019337165897523,
    )
    .with_nnue_god_name(GodName::Mortal)
}
