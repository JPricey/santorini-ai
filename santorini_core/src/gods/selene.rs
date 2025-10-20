use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
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
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves,
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
const IS_DOME_BUILD_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

const IS_DOME_BUILD_MASK: MoveData = 1 << IS_DOME_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
struct SeleneMove(MoveData);

impl Into<GenericMove> for SeleneMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for SeleneMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl SeleneMove {
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

    fn new_dome_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | IS_DOME_BUILD_MASK;

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

    fn is_dome_build(self) -> bool {
        self.0 & IS_DOME_BUILD_MASK != 0
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for SeleneMove {
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
        } else if self.is_dome_build() {
            write!(f, "{}>{}^{}X", move_from, move_to, build,)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build,)
        }
    }
}

impl GodMove for SeleneMove {
    fn move_to_actions(self, board: &BoardState) -> Vec<FullAction> {
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

        if self.get_is_winning() {
            return vec![res];
        }

        let build_position = self.build_position();
        let must_be_dome_build =
            (NEIGHBOR_MAP[self.move_to_position() as usize] & build_position.to_board()).is_empty();

        if !must_be_dome_build {
            res.push(PartialAction::Build(build_position));
        }

        if self.is_dome_build() {
            res.push(PartialAction::Dome(build_position));
        }

        return vec![res];
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        // Check move F worker
        if (board.god_data[player as usize] & worker_move_mask.0) != 0 {
            board.delta_god_data(player, worker_move_mask.0);
        }

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let build_position = self.build_position();
        if self.is_dome_build() {
            board.dome_up(build_position);
        } else {
            board.build_up(build_position);
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_bool(self.is_dome_build());
        helper.get()
    }
}

fn selene_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(selene_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let f_worker_mask = BitBoard(state.board.god_data[player as usize]);
    let mut f_worker_builds = BitBoard::EMPTY;
    if let Some(f_worker_pos) = f_worker_mask.maybe_lsb() {
        f_worker_builds = NEIGHBOR_MAP[f_worker_pos as usize] & !prelude.exactly_level_3;
    }

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let is_f_worker = (worker_start_state.worker_start_mask & f_worker_mask).is_not_empty();

        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, SeleneMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                SeleneMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);

            let mut narrowed_unblocked_squares = unblocked_squares;

            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (worker_end_move_state.worker_end_mask
                    & prelude.key_squares)
                    .is_not_empty() as usize;
                narrowed_unblocked_squares &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            let regular_buildables = narrowed_unblocked_squares
                & prelude.build_mask
                & NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize];

            let dome_buildables = if is_f_worker {
                narrowed_unblocked_squares
                    & NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                    & !prelude.exactly_level_3
            } else {
                f_worker_builds & narrowed_unblocked_squares
            };

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            for worker_build_pos in regular_buildables {
                let new_action = SeleneMove::new_basic_move(
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

            for worker_dome_pos in dome_buildables {
                let new_action = SeleneMove::new_dome_build_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_dome_pos,
                );
                let is_check = {
                    let final_level_3 =
                        prelude.exactly_level_3 & !BitBoard::as_mask(worker_dome_pos);
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

pub const fn build_selene() -> GodPower {
    god_power(
        GodName::Selene,
        build_god_power_movers!(selene_move_gen),
        build_god_power_actions::<SeleneMove>(),
        4758789900555289074,
        17548932275576909220,
    )
    .with_nnue_god_name(GodName::Atlas)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
    .with_flip_god_data_horizontal_fn(flip_horizontal)
    .with_flip_god_data_vertical_fn(flip_vertical)
    .with_flip_god_data_transpose_fn(flip_transpose)
    .with_get_female_worker_mask_fn(get_female_worker_mask)
    .with_placement_type(PlacementType::FemaleWorker)
}
