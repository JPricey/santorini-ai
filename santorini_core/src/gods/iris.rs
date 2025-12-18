use crate::{
    bitboard::{
        BETWEEN_MAPPING, BitBoard, BitboardMapping, NEIGHBOR_MAP, PUSH_MAPPING,
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
        harpies::iris_slide_position,
        move_helpers::{
            GeneratorPreludeState, WorkerEndMoveState, WorkerStartMoveState, build_scored_move,
            get_generator_prelude_state, get_reverse_direction_neighbor_map,
            get_worker_next_build_state, get_worker_start_move_state, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
            restrict_moves_by_affinity_area,
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

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct IrisMove(pub MoveData);

impl GodMove for IrisMove {
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
        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        if let Some(_) = BETWEEN_MAPPING[move_from as usize][move_to as usize] {
            // TODO: check detection to include workers themselves
            BitBoard::MAIN_SECTION_MASK
        } else {
            move_from.to_board() | move_to.to_board()
        }
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for IrisMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for IrisMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl IrisMove {
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

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for IrisMove {
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

fn _iris_get_worker_next_moves<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    all_jumpable_workers: BitBoard,
) -> BitBoard {
    let base_worker_moves =
        prelude.standard_neighbor_map[worker_start_state.worker_start_pos as usize];
    let jump_overs = base_worker_moves & all_jumpable_workers;

    let mut jumping_moves = BitBoard::EMPTY;
    for jump_over_pos in jump_overs {
        let jump_to_pos =
            PUSH_MAPPING[worker_start_state.worker_start_pos as usize][jump_over_pos as usize];
        if let Some(jump_to_pos) = jump_to_pos {
            let jump_to_mask = BitBoard::as_mask(jump_to_pos);
            jumping_moves |= jump_to_mask;
        }
    }

    if MUST_CLIMB {
        let base_move_height_mask = match prelude
            .board
            .get_height(worker_start_state.worker_start_pos)
        {
            0 => prelude.exactly_level_1,
            1 => prelude.exactly_level_2,
            2 => prelude.exactly_level_3,
            3 => return BitBoard::EMPTY,
            _ => unreachable!(),
        };

        let step_up_base_moves = base_worker_moves & base_move_height_mask;
        let rising_jump_moves =
            jumping_moves & prelude.board.height_map[worker_start_state.worker_start_height];

        let worker_moves = (step_up_base_moves | rising_jump_moves)
            & !(prelude.all_workers_and_frozen_mask | prelude.board.height_map[3]);
        worker_moves
    } else {
        if prelude.can_climb {
            let down_mask =
                if prelude.is_down_prevented && worker_start_state.worker_start_height > 0 {
                    !prelude.board.height_map[worker_start_state.worker_start_height - 1]
                } else {
                    BitBoard::EMPTY
                };
            let not_too_high_base_moves = base_worker_moves
                & !prelude.board.height_map[3.min(worker_start_state.worker_start_height + 1)];
            let all_worker_moves = not_too_high_base_moves | jumping_moves;
            let worker_moves = all_worker_moves
                & !(down_mask | prelude.all_workers_and_frozen_mask | prelude.board.height_map[3]);

            restrict_moves_by_affinity_area(
                worker_start_state.worker_start_mask,
                worker_moves,
                prelude.affinity_area,
            )
        } else {
            let all_worker_moves = base_worker_moves | jumping_moves;
            let worker_moves = all_worker_moves
                & !(prelude.board.height_map[worker_start_state.worker_start_height]
                    | prelude.all_workers_and_frozen_mask);
            worker_moves
        }
    }
}

fn _iris_reach_board<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    reverse_map: &BitboardMapping,
    worker_end_move_state: &WorkerEndMoveState,
    jumpable_for_neighbors: BitBoard,
    other_own_jumpable_workers: BitBoard,
    neighbor_reach: BitBoard,
    neighbor_max_height: usize,
) -> BitBoard {
    if prelude.is_against_hypnus {
        let mut reach_board = BitBoard::EMPTY;
        if neighbor_max_height <= worker_end_move_state.worker_end_height {
            reach_board = neighbor_reach;

            for jump_over_pos in other_own_jumpable_workers
                & NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
            {
                let jump_to_pos = PUSH_MAPPING[jump_over_pos as usize]
                    [worker_end_move_state.worker_end_pos as usize];
                if let Some(jump_to_pos) = jump_to_pos {
                    reach_board |= jump_to_pos.to_board();
                }
            }
        }

        if worker_end_move_state.worker_end_height <= neighbor_max_height {
            if worker_end_move_state.worker_end_height < 3 {
                let this_worker_next_turn_moves =
                    prelude.standard_neighbor_map[worker_end_move_state.worker_end_pos as usize];

                if worker_end_move_state.worker_end_height == 2 {
                    reach_board |= this_worker_next_turn_moves;
                }

                let jumpable_neighbors = jumpable_for_neighbors & this_worker_next_turn_moves;
                for jump_over_pos in jumpable_neighbors
                    & prelude.standard_neighbor_map[worker_end_move_state.worker_end_pos as usize]
                {
                    let jump_to_pos = PUSH_MAPPING[worker_end_move_state.worker_end_pos as usize]
                        [jump_over_pos as usize];
                    if let Some(jump_to_pos) = jump_to_pos {
                        reach_board |= jump_to_pos.to_board();
                    }
                }
            }
        }

        reach_board
    } else {
        let mut reach_board = neighbor_reach;
        let this_worker_next_turn_moves =
            prelude.standard_neighbor_map[worker_end_move_state.worker_end_pos as usize];

        if worker_end_move_state.worker_end_height == 2 {
            reach_board |= this_worker_next_turn_moves;
        }

        if worker_end_move_state.worker_end_height < 3 {
            let jumpable_neighbors = jumpable_for_neighbors & this_worker_next_turn_moves;
            for jump_over_pos in jumpable_neighbors
                & prelude.standard_neighbor_map[worker_end_move_state.worker_end_pos as usize]
            {
                let jump_to_pos = PUSH_MAPPING[worker_end_move_state.worker_end_pos as usize]
                    [jump_over_pos as usize];
                if let Some(jump_to_pos) = jump_to_pos {
                    reach_board |= jump_to_pos.to_board();
                }
            }
        }

        for jump_over_pos in
            other_own_jumpable_workers & reverse_map[worker_end_move_state.worker_end_pos as usize]
        {
            let jump_to_pos =
                PUSH_MAPPING[jump_over_pos as usize][worker_end_move_state.worker_end_pos as usize];
            if let Some(jump_to_pos) = jump_to_pos {
                reach_board |= jump_to_pos.to_board();
            }
        }
        reach_board
    }
}

fn _iris_get_worker_end_move_state<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    mut worker_end_pos: Square,
) -> WorkerEndMoveState {
    if prelude.is_against_harpies {
        worker_end_pos =
            iris_slide_position(prelude, worker_start_state.worker_start_pos, worker_end_pos);
    }

    let worker_end_mask = BitBoard::as_mask(worker_end_pos);
    let worker_end_height = prelude.board.get_height(worker_end_pos);
    let is_improving = worker_end_height > worker_start_state.worker_start_height;
    let is_now_lvl_2 = (worker_end_height == 2) as u32;

    WorkerEndMoveState {
        worker_end_pos,
        worker_end_mask,
        worker_end_height,
        is_improving,
        is_now_lvl_2,
    }
}

pub(super) fn iris_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(iris_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    modify_prelude_for_checking_workers::<F>(!prelude.exactly_level_3, &mut prelude);

    let reverse_map = get_reverse_direction_neighbor_map(&prelude);

    let all_jumpable_workers =
        (prelude.own_workers | prelude.oppo_workers) & !prelude.domes_and_frozen;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_moves = _iris_get_worker_next_moves::<MUST_CLIMB>(
            &prelude,
            &worker_start_state,
            all_jumpable_workers,
        );

        if is_mate_only::<F>() || worker_start_state.worker_start_height != 3 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, IrisMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                IrisMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        // TODO: harpies
        let other_own_level_2_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_own_jumpable_workers =
            worker_start_state.other_own_workers & !prelude.board.height_map[2];
        let mut neighbor_max_height = 0;

        let mut neighbor_reach =
            apply_mapping_to_mask(other_own_level_2_workers, prelude.standard_neighbor_map);

        let jumpable_for_neighbors = all_jumpable_workers ^ worker_start_state.worker_start_mask;
        for jump_from_pos in other_own_jumpable_workers {
            let jump_over_neighbors =
                prelude.standard_neighbor_map[jump_from_pos as usize] & jumpable_for_neighbors;
            for jump_over_pos in jump_over_neighbors {
                let jump_to_pos = PUSH_MAPPING[jump_from_pos as usize][jump_over_pos as usize];
                if let Some(jump_to_pos) = jump_to_pos {
                    neighbor_reach |= jump_to_pos.to_board();
                }
            }
            neighbor_max_height = neighbor_max_height.max(prelude.board.get_height(jump_from_pos));
        }

        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                _iris_get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = _iris_reach_board::<F>(
                &prelude,
                &reverse_map,
                &worker_end_move_state,
                jumpable_for_neighbors,
                other_own_jumpable_workers,
                neighbor_reach,
                neighbor_max_height,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let build_mask = worker_build_pos.to_board();
                let new_action = IrisMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board
                        & final_level_3
                        & worker_next_build_state.unblocked_squares
                        & prelude.win_mask;
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

pub const fn build_iris() -> GodPower {
    god_power(
        GodName::Iris,
        build_god_power_movers!(iris_move_gen),
        build_god_power_actions::<IrisMove>(),
        9272470162271642607,
        1980090300899199513,
    )
    .with_nnue_god_name(GodName::Mortal)
}
