use crate::{
    bitboard::{apply_mapping_to_mask, BitBoard},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        build_god_power_actions,
        generic::{
            GenericMove, GodMove, MoveData, MoveGenFlags, ScoredMove, ANY_MOVE_FILTER,
            LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, NULL_MOVE_DATA, POSITION_WIDTH,
        },
        god_power,
        mortal::mortal_move_gen,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_sized_result,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers, push_winning_moves,
            restrict_moves_by_affinity_area, GeneratorPreludeState, WorkerStartMoveState,
        },
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod,
    },
    player::Player,
    search::Heuristic,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const USE_POWER_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const USE_POWER_MASK: GodData = 1 << USE_POWER_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct BellerophonMove(pub MoveData);

impl Into<GenericMove> for BellerophonMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for BellerophonMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl BellerophonMove {
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

    fn new_power_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | USE_POWER_MASK;

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn new_winning_power_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | USE_POWER_MASK
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

    pub(crate) fn is_use_power(self) -> bool {
        (self.0 & USE_POWER_MASK) != 0
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for BellerophonMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            if self.is_use_power() {
                write!(f, "{}>*{}#", move_from, move_to)
            } else {
                write!(f, "{}>{}#", move_from, move_to)
            }
        } else if self.is_use_power() {
            write!(f, "{}>*{}^{}", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for BellerophonMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = vec![PartialAction::SelectWorker(self.move_from_position())];
        if self.is_use_power() {
            res.push(PartialAction::HeroPower(self.move_from_position()));
        }
        res.push(PartialAction::MoveWorker(self.move_to_position().into()));

        if !self.get_is_winning() {
            res.push(PartialAction::Build(self.build_position()));
        }

        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.is_use_power() {
            board.set_god_data(player, 1);
        }

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);
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

// Returns normal, using-power
// Assumes MUST_CLIMB is FALSE. Will handle that situation separate
fn _bellerophon_next_moves<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    start_state: &WorkerStartMoveState,
) -> (BitBoard, BitBoard) {
    let move_mask = prelude.standard_neighbor_map[start_state.worker_start_pos as usize];

    if prelude.can_climb {
        if prelude.is_down_prevented {
            let moving_down_mask = if start_state.worker_start_height > 0 {
                !prelude.board.height_map[start_state.worker_start_height - 1]
            } else {
                BitBoard::EMPTY
            };

            let climb_height = 3.min(start_state.worker_start_height as usize + 1);
            let regular_move_mask = move_mask
                & !(prelude.all_workers_and_frozen_mask
                    | moving_down_mask
                    | prelude.board.height_map[climb_height]);

            let power_climb_height = 3.min(start_state.worker_start_height as usize + 2);
            let power_move_mask = move_mask
                & !(prelude.all_workers_and_frozen_mask
                    | moving_down_mask
                    | prelude.board.height_map[power_climb_height]
                    | regular_move_mask);

            return (regular_move_mask, power_move_mask);
        } else {
            let climb_height = 3.min(start_state.worker_start_height as usize + 1);
            let regular_move_mask = move_mask
                & !(prelude.all_workers_and_frozen_mask | prelude.board.height_map[climb_height]);

            let power_climb_height = 3.min(start_state.worker_start_height as usize + 2);
            let power_move_mask = move_mask
                & !(prelude.all_workers_and_frozen_mask
                    | prelude.board.height_map[power_climb_height]
                    | regular_move_mask);

            (
                restrict_moves_by_affinity_area(
                    start_state.worker_start_mask,
                    regular_move_mask,
                    prelude.affinity_area,
                ),
                restrict_moves_by_affinity_area(
                    start_state.worker_start_mask,
                    power_move_mask,
                    prelude.affinity_area,
                ),
            )
        }
    } else {
        let too_high_mask = prelude.board.height_map[start_state.worker_start_height as usize];
        let open_squares = !(too_high_mask | prelude.all_workers_and_frozen_mask);
        (move_mask & open_squares, BitBoard::EMPTY)
    }
}

fn _bellerophon_reach_board<const HAS_POWER_AVAILABLE: bool>(
    prelude: &GeneratorPreludeState,
    other_reach_board: BitBoard,
    other_max_height: usize,
    worker_end_pos: Square,
    current_height: usize,
    unblocked_squares: BitBoard,
) -> BitBoard {
    let basic_blockers = prelude.win_mask & unblocked_squares;
    if prelude.is_against_hypnus {
        if HAS_POWER_AVAILABLE {
            match current_height {
                0 => BitBoard::EMPTY,
                1 => match other_max_height {
                    0 => BitBoard::EMPTY,
                    1 => {
                        (other_reach_board | prelude.standard_neighbor_map[worker_end_pos as usize])
                            & basic_blockers
                    }
                    _ => prelude.standard_neighbor_map[worker_end_pos as usize] & basic_blockers,
                },
                2 => {
                    if other_max_height == 2 {
                        (other_reach_board | prelude.standard_neighbor_map[worker_end_pos as usize])
                            & basic_blockers
                    } else {
                        other_reach_board & basic_blockers
                    }
                }
                3 => other_reach_board & basic_blockers,
                _ => unreachable!(),
            }
        } else {
            match current_height {
                0 => BitBoard::EMPTY,
                1 => BitBoard::EMPTY,
                2 => {
                    if other_max_height == 2 {
                        (other_reach_board | prelude.standard_neighbor_map[worker_end_pos as usize])
                            & basic_blockers
                    } else {
                        BitBoard::EMPTY
                    }
                }
                3 => other_reach_board & basic_blockers,
                _ => unreachable!(),
            }
        }
    } else {
        if HAS_POWER_AVAILABLE {
            if current_height >= 1 && current_height < 3 {
                (other_reach_board | prelude.standard_neighbor_map[worker_end_pos as usize])
                    & basic_blockers
            } else {
                other_reach_board & basic_blockers
            }
        } else {
            if current_height == 2 {
                (other_reach_board | prelude.standard_neighbor_map[worker_end_pos as usize])
                    & basic_blockers
            } else {
                other_reach_board & basic_blockers
            }
        }
    }
}

fn _bellerophon_must_climb_not_using_power<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
) -> bool {
    let acting_workers = if is_mate_only::<F>() {
        prelude.own_workers & prelude.exactly_level_2
    } else {
        prelude.own_workers
    };

    for worker_start_pos in acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let height_mask = match worker_start_state.worker_start_height {
            0 => prelude.exactly_level_1,
            1 => prelude.exactly_level_2,
            2 => prelude.exactly_level_3,
            3 => BitBoard::EMPTY,
            _ => unreachable!(),
        };

        let mut worker_moves = prelude.standard_neighbor_map
            [worker_start_state.worker_start_pos as usize]
            & height_mask
            & !prelude.all_workers_and_frozen_mask;

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            // Standard moves to level 3
            let standard_moves_to_level_3 = worker_moves & prelude.exactly_level_3;
            if push_winning_moves::<F, BellerophonMove, _>(
                result,
                worker_start_pos,
                standard_moves_to_level_3,
                BellerophonMove::new_winning_move,
            ) {
                return true;
            }
            worker_moves ^= standard_moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_checks = apply_mapping_to_mask(
            worker_start_state.other_own_workers
                & (prelude.exactly_level_1 | prelude.exactly_level_2),
            prelude.standard_neighbor_map,
        );

        // Standard moves
        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = if worker_end_move_state.worker_end_height >= 1 {
                other_checks
                    | prelude.standard_neighbor_map[worker_end_move_state.worker_end_pos as usize]
            } else {
                other_checks
            } & worker_next_build_state.unblocked_squares;

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = BellerophonMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
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

    false
}

fn _bellerophon_must_climb_using_power<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
) {
    let acting_workers = if is_mate_only::<F>() {
        prelude.own_workers & prelude.exactly_level_1
    } else {
        prelude.own_workers & !prelude.board.height_map[1]
    };

    for worker_start_pos in acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let dest_height = if is_mate_only::<F>() || worker_start_state.worker_start_height == 1 {
            prelude.exactly_level_3
        } else {
            prelude.exactly_level_2
        };

        let mut worker_moves = prelude.standard_neighbor_map
            [worker_start_state.worker_start_pos as usize]
            & dest_height
            & !prelude.all_workers_and_frozen_mask;

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 1 {
            let power_moves_to_level_3 = worker_moves & prelude.exactly_level_3;
            if push_winning_moves::<F, BellerophonMove, _>(
                result,
                worker_start_pos,
                power_moves_to_level_3,
                BellerophonMove::new_winning_power_move,
            ) {
                return;
            }
            worker_moves ^= power_moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_checks = apply_mapping_to_mask(
            worker_start_state.other_own_workers & prelude.exactly_level_2,
            prelude.standard_neighbor_map,
        );

        // Power moves
        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = if worker_end_move_state.worker_end_height >= 2 {
                other_checks
                    | prelude.standard_neighbor_map[worker_end_move_state.worker_end_pos as usize]
            } else {
                other_checks
            } & worker_next_build_state.unblocked_squares;

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = BellerophonMove::new_power_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };
                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }
    }
}

fn bellerophon_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let has_power_available = state.board.god_data[player as usize] == 0;
    if !has_power_available {
        return mortal_move_gen::<F, MUST_CLIMB>(state, player, key_squares);
    }
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let mut result = get_sized_result::<F>();

    if MUST_CLIMB {
        unreachable!();
    }

    if prelude.other_god.god_name == GodName::Persephone {
        let did_mate = _bellerophon_must_climb_not_using_power::<F>(&prelude, &mut result);
        if is_stop_on_mate::<F>() && did_mate {
            return result;
        }

        if result.len() > 0 {
            _bellerophon_must_climb_using_power::<F>(&prelude, &mut result);
            return result;
        }

        // Maybe we couldn't find a move because we were filtering moves somehow
        // Try to find a move without filtering... if we can, return nothing since we couldn't meet
        // our initial constraint (if we can't we'll try to meet that constraint without moving up)
        if F & ANY_MOVE_FILTER > 0 {
            _bellerophon_must_climb_not_using_power::<0>(&prelude, &mut result);
            if result.len() > 0 {
                result.clear();

                _bellerophon_must_climb_using_power::<F>(&prelude, &mut result);
                return result;
            }
        }
    }

    let checkable_mask = prelude.exactly_level_2 | prelude.exactly_level_1;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let (mut standard_moves, mut power_moves) =
            _bellerophon_next_moves::<F>(&prelude, &worker_start_state);

        if worker_start_state.worker_start_height < 3 {
            // Standard moves to level 3
            let standard_moves_to_level_3 =
                standard_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, BellerophonMove, _>(
                &mut result,
                worker_start_pos,
                standard_moves_to_level_3,
                BellerophonMove::new_winning_move,
            ) {
                return result;
            }

            // Power moves to level 3
            let power_moves_to_level_3 = power_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, BellerophonMove, _>(
                &mut result,
                worker_start_pos,
                power_moves_to_level_3,
                BellerophonMove::new_winning_power_move,
            ) {
                return result;
            }

            standard_moves ^= standard_moves_to_level_3;
            power_moves ^= power_moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let mut max_other_worker_level = 0;
        let mut power_checks = BitBoard::EMPTY;
        let mut mortal_checks = BitBoard::EMPTY;
        for n_pos in worker_start_state.other_own_workers & prelude.exactly_level_1 {
            max_other_worker_level = 1;
            power_checks |= prelude.standard_neighbor_map[n_pos as usize];
        }
        for n_pos in worker_start_state.other_own_workers & prelude.exactly_level_2 {
            max_other_worker_level = 2;
            mortal_checks |= prelude.standard_neighbor_map[n_pos as usize];
        }
        power_checks |= mortal_checks;

        // Standard moves
        for worker_end_pos in standard_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = _bellerophon_reach_board::<true>(
                &prelude,
                power_checks,
                max_other_worker_level,
                worker_end_move_state.worker_end_pos,
                worker_end_move_state.worker_end_height as usize,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = BellerophonMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
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

        // Power moves
        for worker_end_pos in power_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );
            let reach_board = _bellerophon_reach_board::<false>(
                &prelude,
                mortal_checks,
                max_other_worker_level,
                worker_end_move_state.worker_end_pos,
                worker_end_move_state.worker_end_height as usize,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = BellerophonMove::new_power_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(new_action, is_check, false))
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
    if data == 0 {
        500
    } else {
        0
    }
}

pub const fn build_bellerophon() -> GodPower {
    god_power(
        GodName::Bellerophon,
        build_god_power_movers!(bellerophon_move_gen),
        build_god_power_actions::<BellerophonMove>(),
        5298741033339150823,
        11489085425414648714,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_eval_score_modifier_fn(eval_modifier)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}
