use crate::{
    bitboard::{
        BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP, PUSH_MAPPING, WIND_AWARE_NEIGHBOR_MAP,
        apply_mapping_to_mask,
    },
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::slide_position_with_custom_worker_blocker,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_next_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
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
const KILL_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct BiaMove(pub MoveData);

impl GodMove for BiaMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![PartialAction::SelectWorker(self.move_from_position())];

        if let Some(kill_pos) = self.killed_worker_pos() {
            res.push(PartialAction::new_move_with_kill(
                self.move_to_position(),
                kill_pos,
            ));
        } else {
            res.push(PartialAction::MoveWorker(self.move_to_position().into()));
        }

        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        if let Some(killed_worker_pos) = self.killed_worker_pos() {
            board.worker_xor(!player, BitBoard::as_mask(killed_worker_pos));
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for BiaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for BiaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl BiaMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << KILL_POSITION_OFFSET);

        Self(data)
    }

    fn new_basic_move_with_kill(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        kill_pos: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((kill_pos as MoveData) << KILL_POSITION_OFFSET);

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

    fn killed_worker_pos(self) -> Option<Square> {
        let val = (self.0 >> KILL_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if val == 25 {
            None
        } else {
            Some(Square::from(val))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for BiaMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if let Some(killed_worker_square) = self.killed_worker_pos() {
            write!(
                f,
                "{}>{}x{}^{}",
                move_from, move_to, killed_worker_square, build
            )
        } else if is_win {
            write!(f, "{}>{}#", move_from, move_to)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

pub(super) fn bia_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(bia_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let targetable_oppo_workers = prelude.oppo_workers & !prelude.domes_and_frozen;
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    // TODO: can use non-checking workers to "win" on kills
    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, BiaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                BiaMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for mut worker_end_pos in worker_next_moves.worker_moves {
            let next_square_test =
                PUSH_MAPPING[worker_start_state.worker_start_pos as usize][worker_end_pos as usize];

            if let Some(next_square) = next_square_test {
                let next_mask = BitBoard::as_mask(next_square);
                if (next_mask & targetable_oppo_workers).is_not_empty() {
                    let new_oppo_workers = prelude.oppo_workers ^ next_mask;

                    if prelude.is_against_harpies {
                        worker_end_pos = slide_position_with_custom_worker_blocker(
                            prelude.board,
                            worker_start_state.worker_start_pos,
                            worker_end_pos,
                            prelude.all_workers_and_frozen_mask ^ next_mask,
                        );
                    }

                    let worker_end_mask = BitBoard::as_mask(worker_end_pos);
                    let worker_end_height = prelude.board.get_height(worker_end_pos);
                    let is_now_lvl_2 = (worker_end_height == 2) as u32;

                    if prelude.other_god.is_aphrodite
                        && (prelude.affinity_area & worker_start_state.worker_start_mask)
                            .is_not_empty()
                    {
                        let new_affinity_mask =
                            apply_mapping_to_mask(new_oppo_workers, &INCLUSIVE_NEIGHBOR_MAP);
                        if (worker_end_mask & new_affinity_mask).is_empty() {
                            continue;
                        }
                    }

                    let build_mask = prelude.other_god.get_build_mask(new_oppo_workers)
                        | prelude.exactly_level_3;

                    let unblocked_squares = !((worker_start_state.all_non_moving_workers
                        ^ next_mask)
                        | worker_end_mask
                        | prelude.domes_and_frozen);

                    let all_possible_builds =
                        NEIGHBOR_MAP[worker_end_pos as usize] & unblocked_squares & build_mask;

                    let mut narrowed_builds = all_possible_builds;
                    if is_interact_with_key_squares::<F>() {
                        let is_already_matched =
                            ((worker_end_mask | next_mask) & prelude.key_squares).is_not_empty()
                                as usize;
                        narrowed_builds &=
                            [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
                    }

                    let reach_board = {
                        let next_turn_moves = WIND_AWARE_NEIGHBOR_MAP[prelude.wind_idx]
                            [worker_end_pos as usize]
                            & unblocked_squares;

                        if prelude.is_against_hypnus
                            && (worker_next_moves.other_threatening_workers.count_ones()
                                + is_now_lvl_2)
                                < 2
                        {
                            BitBoard::EMPTY
                        } else {
                            (worker_next_moves.other_threatening_neighbors
                                | (next_turn_moves * is_now_lvl_2))
                                & prelude.win_mask
                                & unblocked_squares
                        }
                    };

                    for worker_build_pos in narrowed_builds {
                        let new_action = BiaMove::new_basic_move_with_kill(
                            worker_start_pos,
                            worker_end_pos,
                            worker_build_pos,
                            next_square,
                        );
                        let is_check = {
                            let final_level_3 = (prelude.exactly_level_2
                                & BitBoard::as_mask(worker_build_pos))
                                | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos));
                            let check_board = reach_board & final_level_3;
                            check_board.is_not_empty()
                        };

                        result.push(build_scored_move::<F, _>(new_action, is_check, true));
                    }

                    continue;
                }
            }

            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            // No kill - behave like a mortal
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = BiaMove::new_basic_move(
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

pub(super) const fn build_bia() -> GodPower {
    god_power(
        GodName::Bia,
        build_god_power_movers!(bia_move_gen),
        build_god_power_actions::<BiaMove>(),
        7857180099000210635,
        6207457018138760746,
    )
    .with_is_placement_priority()
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::{
        fen::parse_fen,
        player::Player,
        search::{SearchContext, WINNING_SCORE_BUFFER, negamax_search},
        search_terminators::DynamicMaxDepthSearchTerminator,
        square::Square,
        transposition_table::TranspositionTable,
    };

    #[test]
    fn test_bia_wins_search_after_kill() {
        let state = parse_fen("00000 00000 00000 00000 00000/1/bia:C3/mortal:C5").unwrap();

        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            new_best_move_callback: Box::new(move |_new_best_move| {}),
            terminator: DynamicMaxDepthSearchTerminator::new(4),
        };
        let search_state = negamax_search(&mut search_context, state);
        assert!(search_state.best_move.unwrap().score > WINNING_SCORE_BUFFER);
    }

    #[test]
    fn test_bia_kills() {
        let state = parse_fen("04040 04040 04040 04440 00000/1/bia:C3/mortal:C5").unwrap();
        let next_states = state.get_next_states();
        assert_eq!(next_states.len(), 2);
        for s in next_states {
            assert!(s.board.workers[1].all_squares().is_empty());
        }
    }

    #[test]
    fn test_bia_places_first_when_second() {
        let state = parse_fen("00000 00000 00000 00000 00000/1/mortal/bia").unwrap();
        let next_states = state.get_next_states_interactive();
        assert!(next_states.len() > 0);
        for s in &next_states {
            assert!(s.state.board.workers[0].is_empty());
            assert!(s.state.board.workers[1].is_not_empty());
        }
    }

    #[test]
    fn test_bia_places_first_when_first() {
        let state = parse_fen("00000 00000 00000 00000 00000/1/bia/mortal").unwrap();
        let next_states = state.get_next_states_interactive();
        assert!(next_states.len() > 0);
        for s in &next_states {
            assert!(s.state.board.workers[0].is_not_empty());
            assert!(s.state.board.workers[1].is_empty());
        }
    }

    #[test]
    fn test_bia_start_second_in_search() {
        let state = parse_fen("00000 00000 00000 00000 00000/1/mortal/bia").unwrap();

        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            new_best_move_callback: Box::new(move |_new_best_move| {}),
            terminator: DynamicMaxDepthSearchTerminator::new(1),
        };
        let search_state = negamax_search(&mut search_context, state);
        let child_board = search_state.best_move.unwrap().child_state.board;
        assert_eq!(child_board.current_player, Player::Two);
        assert!(child_board.workers[0].is_empty());
        assert!(child_board.workers[1].is_not_empty());
    }

    #[test]
    fn test_bia_start_first_in_search() {
        let state = parse_fen("00000 00000 00000 00000 00000/1/bia/mortal").unwrap();

        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            new_best_move_callback: Box::new(move |_new_best_move| {}),
            terminator: DynamicMaxDepthSearchTerminator::new(1),
        };
        let search_state = negamax_search(&mut search_context, state);
        let child_board = search_state.best_move.unwrap().child_state.board;
        assert_eq!(child_board.current_player, Player::Two);
        assert!(child_board.workers[0].is_not_empty());
        assert!(child_board.workers[1].is_empty());
    }

    #[test]
    fn test_bia_doesnt_kill_clio_on_coin() {
        let state = parse_fen("04040 04040 04040 04440 00000/1/bia:C3/clio[0|C5]:C5").unwrap();
        let next_states = state.get_next_states();
        assert_eq!(next_states.len(), 1);

        let next_state = next_states[0].clone();
        assert_eq!(next_state.board.workers[1].all_squares(), vec![Square::C5]);
    }
}
