use crate::{
    bitboard::{BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK, NEIGHBOR_MAP},
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
            is_interact_with_key_squares, is_mate_only, is_stop_on_mate,
            modify_prelude_for_checking_workers, push_winning_moves,
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

const DOME_BUILD_1: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const DOME_BUILD_2: usize = DOME_BUILD_1 + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PolyphemusMove(pub MoveData);

impl Into<GenericMove> for PolyphemusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for PolyphemusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl PolyphemusMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << DOME_BUILD_1);

        Self(data)
    }

    fn new_power_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        dome1: Square,
        dome2: Option<Square>,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((dome1 as MoveData) << DOME_BUILD_1)
            | ((dome2.map_or(25, |s| s as MoveData)) << DOME_BUILD_2);

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

    pub fn dome_1(self) -> Option<Square> {
        let value = (self.0 >> DOME_BUILD_1) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn dome_2(self) -> Option<Square> {
        let value = (self.0 >> DOME_BUILD_2) as u8 & LOWER_POSITION_MASK;
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

impl std::fmt::Debug for PolyphemusMove {
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
        } else if let Some(d1) = self.dome_1() {
            if let Some(d2) = self.dome_2() {
                write!(f, "{}>{}^{} X{}X{}", move_from, move_to, build, d1, d2)
            } else {
                write!(f, "{}>{}^{} X{}", move_from, move_to, build, d1)
            }
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build,)
        }
    }
}

impl GodMove for PolyphemusMove {
    fn move_to_actions(self, _board: &BoardState, _player: Player, _other_god: StaticGod) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];

        if self.get_is_winning() {
            return vec![res];
        }

        let build_position = self.build_position();
        res.push(PartialAction::Build(build_position));

        if let Some(d1) = self.dome_1() {
            res.push(PartialAction::HeroPower(self.move_to_position()));

            if let Some(d2) = self.dome_2() {
                let mut res_clone = res.clone();
                res.push(PartialAction::Dome(d1));
                res.push(PartialAction::Dome(d2));

                res_clone.push(PartialAction::Dome(d2));
                res_clone.push(PartialAction::Dome(d1));

                vec![res, res_clone]
            } else {
                res.push(PartialAction::Dome(d1));
                vec![res]
            }
        } else {
            vec![res]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);

        if let Some(d1) = self.dome_1() {
            board.set_god_data(player, 1);
            board.dome_up(d1);

            if let Some(d2) = self.dome_2() {
                board.dome_up(d2);
            }
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
        helper.add_maybe_square_with_height(board, self.dome_1());
        helper.add_maybe_square_with_height(board, self.dome_2());
        helper.get()
    }
}

fn polyphemus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(polyphemus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let has_power_available = state.board.god_data[player as usize] == 0;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, PolyphemusMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                PolyphemusMove::new_winning_move,
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

            if has_power_available {
                for worker_build_pos in all_possible_builds {
                    let build_mask = worker_build_pos.to_board();
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);

                    let did_interact_yet = if is_interact_with_key_squares::<F>() {
                        (key_squares & (worker_end_move_state.worker_end_mask | build_mask))
                            .is_not_empty()
                    } else {
                        true
                    };

                    let did_build_to_dome = prelude.board.get_height(worker_build_pos) == 3;

                    if did_interact_yet {
                        let is_check = {
                            let check_board = reach_board & final_level_3;
                            check_board.is_not_empty()
                        };
                        let new_action = PolyphemusMove::new_basic_move(
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

                    let mut possible_domes = unblocked_squares
                        & BitBoard::MAIN_SECTION_MASK
                        & !(build_mask & prelude.exactly_level_3);

                    if is_stop_on_mate::<F>() && did_build_to_dome {
                        possible_domes &= !(all_possible_builds
                            & prelude.exactly_level_3
                            & !LOWER_SQUARES_EXCLUSIVE_MASK[worker_build_pos as usize]);
                    }

                    for d1 in possible_domes {
                        let d1_mask = d1.to_board();

                        let did_interact_yet_d1 = if is_interact_with_key_squares::<F>() {
                            did_interact_yet | (key_squares & (d1_mask)).is_not_empty()
                        } else {
                            true
                        };

                        let did_pass_dupe_check = if is_stop_on_mate::<F>() {
                            if did_build_to_dome & (d1_mask & all_possible_builds).is_not_empty() {
                                d1 > worker_build_pos
                            } else {
                                true
                            }
                        } else {
                            true
                        };

                        if did_interact_yet_d1 && did_pass_dupe_check {
                            let is_check = {
                                let final_level_3_d1 = final_level_3 & !d1_mask; // Domed square can't be level 3
                                let check_board = reach_board & final_level_3_d1;
                                check_board.is_not_empty()
                            };
                            let new_action = PolyphemusMove::new_power_move(
                                worker_start_pos,
                                worker_end_move_state.worker_end_pos,
                                worker_build_pos,
                                d1,
                                None,
                            );

                            result.push(build_scored_move::<F, _>(
                                new_action,
                                is_check,
                                worker_end_move_state.is_improving,
                            ));
                        }

                        let mut possible_squares_2 = if is_stop_on_mate::<F>()
                            && (all_possible_builds & d1_mask).is_not_empty()
                        {
                            // Dupe blocker
                            // If you domed your own build, you can't dome any of your other builds
                            // -> makes it so that when you dome (d1, d2), you cant have both of
                            // those squares have been your build too, which results in dupe states
                            possible_domes & LOWER_SQUARES_EXCLUSIVE_MASK[d1 as usize] & !build_mask
                        } else {
                            possible_domes & LOWER_SQUARES_EXCLUSIVE_MASK[d1 as usize]
                        };

                        if is_interact_with_key_squares::<F>() && !did_interact_yet_d1 {
                            possible_squares_2 &= key_squares;
                            if possible_squares_2.is_empty() {
                                break;
                            }
                        }

                        for d2 in possible_squares_2 {
                            let d2_mask = d2.to_board();

                            let is_check = {
                                let final_level_3_d2 = final_level_3 & !(d1_mask | d2_mask);
                                let check_board = reach_board & final_level_3_d2;
                                check_board.is_not_empty()
                            };
                            let new_action = PolyphemusMove::new_power_move(
                                worker_start_pos,
                                worker_end_move_state.worker_end_pos,
                                worker_build_pos,
                                d1,
                                Some(d2),
                            );

                            result.push(build_scored_move::<F, _>(
                                new_action,
                                is_check,
                                worker_end_move_state.is_improving,
                            ));
                        }
                    }
                }
            } else {
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
                    let new_action = PolyphemusMove::new_basic_move(
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
    if data == 0 { 300 } else { 0 }
}

pub const fn build_polyphemus() -> GodPower {
    god_power(
        GodName::Polyphemus,
        build_god_power_movers!(polyphemus_move_gen),
        build_god_power_actions::<PolyphemusMove>(),
        18142252980210509973,
        12346902543242196568,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_eval_score_modifier_fn(eval_modifier)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}
