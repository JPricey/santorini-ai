use crate::{
    bitboard::{
        BitBoard, DIRECTION_MAPPING, WIND_AWARE_NEIGHBOR_MAP, WRAPPING_DIRECTION_MAPPING,
        apply_mapping_to_mask,
    },
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    direction::{Direction, direction_idx_to_reverse},
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            GeneratorPreludeState, WorkerNextMoveState, build_scored_move, get_basic_moves,
            get_generator_prelude_state, get_standard_reach_board_with_custom_wind,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
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
const WIND_DIRECTION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub(super) struct AeolusMove(pub MoveData);

impl GodMove for AeolusMove {
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
            PartialAction::SetWindDirection(self.wind_direction()),
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

        board.set_god_data(board.current_player, self.wind_direction_idx() as u32);
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_value(self.wind_direction_idx() as usize, 9);
        helper.get()
    }
}

impl Into<GenericMove> for AeolusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for AeolusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl AeolusMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        wind_direction_idx: u8,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((wind_direction_idx as MoveData) << WIND_DIRECTION_OFFSET);

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

    pub fn wind_direction_idx(self) -> u8 {
        (self.0 >> WIND_DIRECTION_OFFSET) as u8 & LOWER_POSITION_MASK
    }

    pub fn wind_direction(self) -> Option<Direction> {
        match self.wind_direction_idx() {
            0 => None,
            x => Some(Direction::from_u8(x - 1)),
        }
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for AeolusMove {
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
            write!(
                f,
                "{}>{}^{} w={}",
                move_from,
                move_to,
                build,
                self.wind_direction()
                    .map_or("-".to_string(), |d| d.to_string())
            )
        }
    }
}

fn aeolus_move_gen_with_next_wind_direction<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    next_wind_idx: usize,
) -> bool {
    let mut did_interact_with_wind = false;
    if is_interact_with_key_squares::<F>() && next_wind_idx > 0 && next_wind_idx != prelude.wind_idx
    {
        let other_god = prelude.other_god.god_name;
        if other_god == GodName::Artemis {
            let oppo = prelude.oppo_workers & prelude.key_squares;
            let wins = prelude.key_squares & prelude.exactly_level_3;

            let non_oppo = prelude.key_squares ^ oppo;
            let non_wins = prelude.key_squares ^ wins;

            for key_square in oppo {
                if let Some(wind_square) = DIRECTION_MAPPING[next_wind_idx - 1][key_square as usize]
                {
                    let wind_mask = BitBoard::as_mask(wind_square);
                    if (wind_mask & non_oppo).is_not_empty() {
                        did_interact_with_wind = true;
                        break;
                    }
                }
            }

            if !did_interact_with_wind {
                let reverse_wind = direction_idx_to_reverse(next_wind_idx) - 1;
                for key_square in wins {
                    if let Some(wind_square) = DIRECTION_MAPPING[reverse_wind][key_square as usize]
                    {
                        let wind_mask = BitBoard::as_mask(wind_square);
                        if (wind_mask & non_wins).is_not_empty() {
                            did_interact_with_wind = true;
                            break;
                        }
                    }
                }
            }
        } else if other_god == GodName::Urania {
            let oppo_workers = prelude.key_squares & prelude.oppo_workers;
            let win_spots = prelude.key_squares ^ oppo_workers;
            for key_square in oppo_workers {
                let wind_square =
                    WRAPPING_DIRECTION_MAPPING[next_wind_idx - 1][key_square as usize];

                let wind_mask = BitBoard::as_mask(wind_square);
                if (wind_mask & win_spots).is_not_empty() {
                    did_interact_with_wind = true;
                    break;
                }
            }
        } else {
            let oppo_workers = prelude.key_squares & prelude.oppo_workers;
            let win_spots = prelude.key_squares ^ oppo_workers;
            for key_square in oppo_workers {
                if let Some(wind_square) = DIRECTION_MAPPING[next_wind_idx - 1][key_square as usize]
                {
                    let wind_mask = BitBoard::as_mask(wind_square);
                    if (wind_mask & win_spots).is_not_empty() {
                        did_interact_with_wind = true;
                        break;
                    }
                }
            }
        }
    }

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_moves = get_basic_moves::<MUST_CLIMB>(prelude, &worker_start_state);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, AeolusMove, _>(
                result,
                worker_start_pos,
                moves_to_level_3,
                AeolusMove::new_winning_move,
            ) {
                return true;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers =
            worker_start_state.other_own_workers & prelude.exactly_level_2;
        let other_threatening_neighbors = apply_mapping_to_mask(
            other_threatening_workers,
            &WIND_AWARE_NEIGHBOR_MAP[next_wind_idx],
        );
        let worker_next_moves = WorkerNextMoveState {
            other_threatening_workers,
            other_threatening_neighbors,
            worker_moves,
        };

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );
            let reach_board = get_standard_reach_board_with_custom_wind::<F>(
                &prelude,
                next_wind_idx,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            let builds_to_try = if did_interact_with_wind {
                worker_next_build_state.all_possible_builds
            } else {
                worker_next_build_state.narrowed_builds
            };

            for worker_build_pos in builds_to_try {
                let new_action = AeolusMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                    next_wind_idx as u8,
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
                ));
            }
        }
    }

    false
}

pub(super) fn aeolus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(aeolus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let wind_direction_idx = state.board.god_data[player as usize] as usize;
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    prelude.wind_idx = wind_direction_idx as usize;

    modify_prelude_for_checking_workers::<F>(prelude.exactly_level_2, &mut prelude);

    if wind_direction_idx == 0 {
        for d in 0..=8 {
            if aeolus_move_gen_with_next_wind_direction::<F, MUST_CLIMB>(&prelude, &mut result, d) {
                return result;
            }
        }
    } else {
        if aeolus_move_gen_with_next_wind_direction::<F, MUST_CLIMB>(&prelude, &mut result, 0) {
            return result;
        }
        if aeolus_move_gen_with_next_wind_direction::<F, MUST_CLIMB>(
            &prelude,
            &mut result,
            wind_direction_idx,
        ) {
            return result;
        }
    }

    result
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    if data.trim().is_empty() {
        return Ok(0);
    }

    let direction: Direction = data.parse().map_err(|e| format!("{:?}", e))?;

    Ok(direction as u32 + 1)
}

fn stringify_god_data(data: GodData) -> Option<String> {
    if data == 0 {
        return None;
    }

    Some(Direction::from_u8(data as u8 - 1).to_string())
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    let god_data = board.god_data[player as usize];
    let wind_direction_str =
        stringify_god_data(god_data).map_or("None".to_string(), |w| w.to_uppercase());
    Some(format!("Preventing: {}", wind_direction_str))
}

fn get_wind_idx(board: &BoardState, player: Player) -> usize {
    board.god_data[player as usize] as usize
}

pub const fn build_aeolus() -> GodPower {
    god_power(
        GodName::Aeolus,
        build_god_power_movers!(aeolus_move_gen),
        build_god_power_actions::<AeolusMove>(),
        12246185600298435959,
        13250172022449743639,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_get_wind_idx_fn(get_wind_idx)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}

#[cfg(test)]
mod tests {
    use crate::{
        fen::parse_fen,
        gods::ALL_GODS_BY_ID,
        matchup::{Matchup, is_matchup_banned},
    };

    use super::*;

    #[test]
    fn test_all_gods_respect_aeolus() {
        for god in ALL_GODS_BY_ID {
            let god_name = god.god_name;
            let matchup = Matchup::new(god_name, GodName::Aeolus);
            if is_matchup_banned(&matchup) {
                eprintln!("skipping banned matchup: {}", matchup);
                continue;
            }

            let state = parse_fen(&format!(
                "04040 04040 04440 00000 00000/1/{}:C4/aeolus[n]:E1,E2",
                god_name
            ))
            .unwrap();

            let moves = god.get_all_moves(&state, Player::One);
            for action in &moves {
                eprintln!("{:?} {}", god_name, god.stringify_move(action.action));
            }
            let expected_moves = if god_name == GodName::Hermes { 1 } else { 0 };

            assert_eq!(moves.len(), expected_moves);
        }
    }
}
