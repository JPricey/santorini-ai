use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, WIND_AWARE_NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        build_god_power_actions, generic::{
            GenericMove, GodMove, MoveData, MoveGenFlags, ScoredMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, NULL_MOVE_DATA, POSITION_WIDTH
        }, god_power, move_helpers::{
            build_scored_move, get_basic_moves, get_generator_prelude_state,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
            is_mate_only, is_stop_on_mate, push_winning_moves,
        }, FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod
    },
    persephone_check_result,
    placement::PlacementType,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const EXTRA_WIN_MASK_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct ErosMove(pub MoveData);

impl GodMove for ErosMove {
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

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position())
            | BitBoard::as_mask(self.move_to_position())
            | self.extra_eros_worker_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for ErosMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ErosMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ErosMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | (25 << EXTRA_WIN_MASK_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn new_winning_eros_move(
        move_from_position: Square,
        move_to_position: Square,
        other_worker_pos: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((other_worker_pos as MoveData) << EXTRA_WIN_MASK_OFFSET)
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

    pub fn extra_eros_worker_mask(self) -> BitBoard {
        BitBoard(1 << ((self.0 >> EXTRA_WIN_MASK_OFFSET) as u8 & LOWER_POSITION_MASK))
            & BitBoard::MAIN_SECTION_MASK
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ErosMove {
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

pub(super) fn eros_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(eros_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let neighbor_map = &WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx];

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut other_worker_pos = Square::A1;
        let mut other_lvl_1_worker_neighbors = BitBoard::EMPTY;
        let mut eros_win_spots: BitBoard = BitBoard::EMPTY;

        let mut other_any_height_workers_reach = BitBoard::EMPTY;
        let mut other_lvl_2_workers_reach = BitBoard::EMPTY;
        let mut has_lvl_2_others = false;

        if let Some(other) = worker_start_state.other_own_workers.maybe_lsb() {
            other_worker_pos = other;
            other_any_height_workers_reach = neighbor_map[other as usize];

            let other_worker_height = prelude.board.get_height(other_worker_pos);
            if other_worker_height == 1 {
                other_lvl_1_worker_neighbors = NEIGHBOR_MAP[other_worker_pos as usize];
                eros_win_spots =
                    other_lvl_1_worker_neighbors & prelude.exactly_level_1 & prelude.win_mask;
            } else if other_worker_height == 2 {
                has_lvl_2_others = true;
                other_lvl_2_workers_reach = other_any_height_workers_reach;
            }
        }

        let mut worker_moves = get_basic_moves::<MUST_CLIMB>(&prelude, &worker_start_state);

        {
            let moves_to_eros_wins = worker_moves & eros_win_spots;
            for end_pos in moves_to_eros_wins {
                let new_action =
                    ErosMove::new_winning_eros_move(worker_start_pos, end_pos, other_worker_pos);
                result.push(ScoredMove::new_winning_move(new_action.into()));
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }

            worker_moves ^= moves_to_eros_wins;
        }

        if worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, ErosMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                ErosMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let eros_wins_from_this_worker = if prelude
                .board
                .get_height(worker_end_move_state.worker_end_pos)
                == 1
            {
                NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
            } else {
                BitBoard::EMPTY
            };

            let this_worker_next_turn_moves = WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx]
                [worker_end_move_state.worker_end_pos as usize];

            let lvl_3_reach_board;
            let mut can_eros_win = true;

            if prelude.is_against_hypnus {
                if worker_end_move_state.is_now_lvl_2 > 0 && has_lvl_2_others {
                    lvl_3_reach_board = other_lvl_2_workers_reach | this_worker_next_turn_moves
                } else {
                    lvl_3_reach_board = BitBoard::EMPTY
                }

                can_eros_win = !(worker_end_move_state.is_now_lvl_2 > 0 || has_lvl_2_others)
            } else {
                if prelude.is_down_prevented {
                    can_eros_win = !(worker_end_move_state.is_now_lvl_2 > 0 || has_lvl_2_others);
                }

                if worker_end_move_state.is_now_lvl_2 > 0 {
                    lvl_3_reach_board = other_lvl_2_workers_reach | this_worker_next_turn_moves
                } else {
                    lvl_3_reach_board = other_lvl_2_workers_reach
                }
            };

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = ErosMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos));
                    let check_board = lvl_3_reach_board & final_level_3;

                    let final_level_1 = (prelude.exactly_level_0
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_1 & !BitBoard::as_mask(worker_build_pos));

                    let this_worker_move_into_win =
                        this_worker_next_turn_moves & other_lvl_1_worker_neighbors;
                    let other_worker_move_into_this_win =
                        eros_wins_from_this_worker & other_any_height_workers_reach;

                    let eros_wins_total = if can_eros_win {
                        (this_worker_move_into_win | other_worker_move_into_this_win)
                            & final_level_1
                    } else {
                        BitBoard::EMPTY
                    };

                    ((eros_wins_total | check_board)
                        & prelude.win_mask
                        & worker_next_build_state.unblocked_squares)
                        .is_not_empty()
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

pub const fn build_eros() -> GodPower {
    god_power(
        GodName::Eros,
        build_god_power_movers!(eros_move_gen),
        build_god_power_actions::<ErosMove>(),
        1623570476180869580,
        6256749897107858133,
    )
    .with_placement_type(PlacementType::PerimeterOpposite)
    .with_nnue_god_name(GodName::Maenads)
}
