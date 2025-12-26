use crate::{
    bitboard::{BitBoard, DIAGONAL_ONLY_NEIGHBOR_MAP, NEIGHBOR_MAP},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            WorkerNextMoveState, build_scored_move, get_basic_moves_from_with_two_movement_maps,
            get_generator_prelude_state, get_standard_reach_board_with_extra_move_map,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    placement::PlacementType,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct HippolytaMove(MoveData);

impl Into<GenericMove> for HippolytaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for HippolytaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl HippolytaMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
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
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for HippolytaMove {
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
            write!(f, "{}>{}^{}", move_from, move_to, build,)
        }
    }
}

impl GodMove for HippolytaMove {
    fn move_to_actions(self, board: &BoardState, _player: Player, _other_god: StaticGod) -> Vec<FullAction> {
        let move_from = self.move_from_position();
        let mut res = vec![PartialAction::SelectWorker(move_from)];
        let is_female =
            (BitBoard::as_mask(move_from).0 & board.god_data[board.current_player as usize]) != 0;
        if is_female {
            res.push(PartialAction::new_move_female_worker(
                self.move_to_position(),
            ));
        } else {
            res.push(PartialAction::MoveWorker(self.move_to_position().into()));
        }

        if !self.get_is_winning() {
            res.push(PartialAction::Build(self.build_position()));
        }
        return vec![res];
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        // Check move F worker
        if (board.god_data[player as usize] & worker_move_mask.0) != 0 {
            board.xor_god_data(player, worker_move_mask.0);
        }

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

fn hippolyta_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(hippolyta_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let f_worker_mask = BitBoard(state.board.god_data[player as usize]);
    let movement_map_by_is_f = [&DIAGONAL_ONLY_NEIGHBOR_MAP, &NEIGHBOR_MAP];

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let is_f_worker = (worker_start_state.worker_start_mask & f_worker_mask).is_not_empty();
        let this_worker_move_map = movement_map_by_is_f[is_f_worker as usize];

        let mut worker_moves = get_basic_moves_from_with_two_movement_maps::<MUST_CLIMB>(
            &prelude,
            this_worker_move_map,
            worker_start_state.worker_start_pos,
            worker_start_state.worker_start_mask,
            worker_start_state.worker_start_height,
            prelude.all_workers_and_frozen_mask,
        );

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, HippolytaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                HippolytaMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let mut other_threatening_neighbors = BitBoard::EMPTY;

        for pos in other_threatening_workers {
            let other_worker_is_f = (pos.to_board() & f_worker_mask).is_not_empty();
            let other_worker_movements = movement_map_by_is_f[other_worker_is_f as usize];
            other_threatening_neighbors |=
                prelude.standard_neighbor_map[pos as usize] & other_worker_movements[pos as usize];
        }

        let worker_next_moves = WorkerNextMoveState {
            other_threatening_workers,
            other_threatening_neighbors,
            worker_moves,
        };

        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = get_standard_reach_board_with_extra_move_map::<F>(
                &prelude,
                this_worker_move_map,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = HippolytaMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos));
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

fn parse_god_data(data: &str) -> Result<GodData, String> {
    if data == "" {
        return Ok(0);
    }

    data.parse()
        .map(|s: Square| BitBoard::as_mask(s).0 as GodData)
        .map_err(|e| format!("{:?}", e))
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        x => Some(BitBoard(x).lsb().to_string()),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => Some("No Female Worker".to_string()),
        x => Some(format!("Female worker at {:?}", BitBoard(x).lsb())),
    }
}

fn get_female_worker_mask(board: &BoardState, player: Player) -> BitBoard {
    BitBoard(board.god_data[player as usize])
}

fn flip_horizontal(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_horizontal().0 as GodData
}

fn flip_vertical(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_vertical().0 as GodData
}

fn flip_transpose(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_transpose().0 as GodData
}

pub const fn build_hippolyta() -> GodPower {
    god_power(
        GodName::Hippolyta,
        build_god_power_movers!(hippolyta_move_gen),
        build_god_power_actions::<HippolytaMove>(),
        1007433104289952955,
        6338572412622910049,
    )
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
    .with_flip_god_data_horizontal_fn(flip_horizontal)
    .with_flip_god_data_vertical_fn(flip_vertical)
    .with_flip_god_data_transpose_fn(flip_transpose)
    .with_get_female_worker_mask_fn(get_female_worker_mask)
    .with_placement_type(PlacementType::FemaleWorker)
}
