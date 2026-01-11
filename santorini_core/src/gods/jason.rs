use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, PERIMETER_SPACES_MASK, apply_mapping_to_mask},
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
    player::Player,
    search::Heuristic,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const PLACE_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct JasonMove(pub MoveData);

impl Into<GenericMove> for JasonMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for JasonMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl JasonMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << PLACE_POSITION_OFFSET);

        Self(data)
    }

    fn new_power_move(place_position: Square, build_position: Square) -> Self {
        let data: MoveData = ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((place_position as MoveData) << PLACE_POSITION_OFFSET);

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
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_place_position(self) -> Option<Square> {
        let value = (self.0 >> PLACE_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for JasonMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let is_win = self.get_is_winning();

        if is_win {
            let move_from = self.move_from_position();
            let move_to = self.move_to_position();
            write!(f, "{}>{}#", move_from, move_to)
        } else if let Some(place_pos) = self.maybe_place_position() {
            let build = self.build_position();
            write!(f, "+{}^{}", place_pos, build)
        } else {
            let move_from = self.move_from_position();
            let move_to = self.move_to_position();
            let build = self.build_position();
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for JasonMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        if self.get_is_winning() {
            return vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position().into()),
            ]];
        }

        if let Some(place_pos) = self.maybe_place_position() {
            vec![vec![
                PartialAction::HeroPower(place_pos),
                PartialAction::Build(self.build_position()),
            ]]
        } else {
            vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position().into()),
                PartialAction::Build(self.build_position()),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        if self.get_is_winning() {
            let worker_move_mask = self.move_mask();
            board.worker_xor(player, worker_move_mask);
            board.set_winner(player);
            return;
        }

        if let Some(place_pos) = self.maybe_place_position() {
            board.set_god_data(player, 1);
            board.worker_xor(player, place_pos.to_board());
            board.build_up(self.build_position());
        } else {
            let worker_move_mask = self.move_mask();
            board.worker_xor(player, worker_move_mask);
            board.build_up(self.build_position());
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        if let Some(place_position) = self.maybe_place_position() {
            helper.add_square_with_height(board, place_position);
        } else {
            helper.add_square_with_height(board, self.move_from_position());
            helper.add_square_with_height(board, self.move_to_position());
        }
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

fn jason_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(jason_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

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
            if push_winning_moves::<F, JasonMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                JasonMove::new_winning_move,
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
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;

            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (worker_end_move_state.worker_end_mask
                    & prelude.key_squares)
                    .is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            for worker_build_pos in narrowed_builds {
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };
                let new_action = JasonMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ));
            }
        }
    }

    let has_power_available = state.board.god_data[player as usize] == 0;
    if has_power_available && !is_mate_only::<F>() {
        let unblocked_squares = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);
        let valid_placements = PERIMETER_SPACES_MASK & prelude.exactly_level_0 & unblocked_squares;

        let threatening_workers = prelude.own_workers & prelude.exactly_level_2;
        let reach_board = if prelude.is_against_hypnus && threatening_workers.count_ones() < 2 {
            BitBoard::EMPTY
        } else {
            apply_mapping_to_mask(threatening_workers, prelude.standard_neighbor_map)
                & prelude.win_mask
                & unblocked_squares
        };

        for place_pos in valid_placements {
            let place_mask = place_pos.to_board();

            let all_possible_builds =
                NEIGHBOR_MAP[place_pos as usize] & unblocked_squares & prelude.build_mask;

            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (place_mask & prelude.key_squares).is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            for build_pos in narrowed_builds {
                let build_mask = build_pos.to_board();

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                let new_action = JasonMove::new_power_move(place_pos, build_pos);

                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }
    }

    result
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(0),
        "x" | "X" => Ok(1),
        _ => Err(format!("Must be either empty string or x")),
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        _ => Some(format!("x")),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => Some(format!("Power available")),
        _ => Some(format!("Power used")),
    }
}

fn eval_modifier(data: GodData) -> Heuristic {
    if data == 0 { 50 } else { 0 }
}

pub const fn build_jason() -> GodPower {
    god_power(
        GodName::Jason,
        build_god_power_movers!(jason_move_gen),
        build_god_power_actions::<JasonMove>(),
        7892341056789234105,
        14567890123456789012,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_eval_score_modifier_fn(eval_modifier)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}
