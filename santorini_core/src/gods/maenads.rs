use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, PUSH_MAPPING},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, PartialAction, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_next_move_state,
            get_worker_start_move_state, is_mate_only, is_stop_on_mate, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const IS_DANCE_WIN_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const IS_DANCE_WIN_BIT: MoveData = (1 as MoveData) << IS_DANCE_WIN_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MaenadsMove(pub MoveData);

impl GodMove for MaenadsMove {
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

    fn make_move(self, board: &mut BoardState, player: Player) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        if self.get_is_dance_win() {
            // TODO: make more specific
            BitBoard::MAIN_SECTION_MASK
        } else {
            BitBoard::as_mask(self.move_from_position())
                | BitBoard::as_mask(self.move_to_position())
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

impl Into<GenericMove> for MaenadsMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for MaenadsMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl MaenadsMove {
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
            | ((25 as MoveData) << BUILD_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    fn new_winning_dance_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | IS_DANCE_WIN_BIT
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
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }

    fn get_is_dance_win(&self) -> bool {
        (self.0 & IS_DANCE_WIN_BIT) != 0
    }
}

impl std::fmt::Debug for MaenadsMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let is_win = self.get_is_winning();

        if is_win {
            write!(f, "{}>{}#", move_from, move_to)
        } else {
            let build = self.build_position();
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

pub(super) fn maenads_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(maenads_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);

    let mut mad_spots = BitBoard::EMPTY;
    for worker_pos in prelude.own_workers {
        let oppo_neighbors = NEIGHBOR_MAP[worker_pos as usize] & prelude.oppo_workers;
        for oppo_neighbor in oppo_neighbors {
            if let Some(other_side_pos) = PUSH_MAPPING[worker_pos as usize][oppo_neighbor as usize]
            {
                mad_spots |= BitBoard::as_mask(other_side_pos);
            }
        }
    }

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves = get_worker_next_move_state::<MUST_CLIMB>(
            &prelude,
            &worker_start_state,
            prelude.exactly_level_2,
        );

        // Win by climbing
        if worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, MaenadsMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                MaenadsMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        // Win by dancing
        if !prelude.is_against_harpies {
            let dance_wins = worker_next_moves.worker_moves & mad_spots & prelude.win_mask;

            if dance_wins.is_not_empty() {
                let unblocked_squares =
                    !(worker_start_state.all_non_moving_workers | prelude.domes_and_frozen);

                for end_pos in dance_wins {
                    let all_possible_builds =
                        NEIGHBOR_MAP[end_pos as usize] & unblocked_squares & prelude.build_mask;

                    if all_possible_builds.is_not_empty() {
                        if push_winning_moves::<F, MaenadsMove, _>(
                            &mut result,
                            worker_start_pos,
                            dance_wins,
                            MaenadsMove::new_winning_dance_move,
                        ) {
                            return result;
                        }
                    }
                }

                worker_next_moves.worker_moves ^= dance_wins;
            }
        }

        if is_mate_only::<F>() && !prelude.is_against_harpies {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            if prelude.is_against_harpies {
                if (worker_end_move_state.worker_end_mask & mad_spots).is_not_empty() {
                    let new_action = MaenadsMove::new_winning_dance_move(
                        worker_start_state.worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                    );
                    result.push(ScoredMove::new_winning_move(new_action.into()));
                    if is_stop_on_mate::<F>() {
                        return result;
                    }
                    continue;
                }

                if is_mate_only::<F>() {
                    continue;
                }
            }

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
                let new_action = MaenadsMove::new_basic_move(
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

pub const fn build_maenads() -> GodPower {
    god_power(
        GodName::Maenads,
        build_god_power_movers!(maenads_move_gen),
        build_god_power_actions::<MaenadsMove>(),
        6024874840407544606,
        8949450171891062378,
    )
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::fen::parse_fen;

    use super::*;

    #[test]
    fn test_maenads_must_slide_vs_harpies() {
        let fen = "/1/maenads:B3,C4/harpies:C3";
        let state = parse_fen(fen).unwrap();
        let maenads = GodName::Maenads.to_power();

        let next_states = maenads.get_all_next_states(&state);
        for s in next_states {
            assert_eq!(
                s.get_winner(),
                None,
                "Maenads won in impossible way vs harpies: {:?}",
                FullGameState::new(s, [maenads, GodName::Harpies.to_power()])
            );
        }
    }

    #[test]
    fn test_maenads_slide_win_vs_harpies() {
        let fen = "/1/maenads:A1,C5/harpies:B1";
        let state = parse_fen(fen).unwrap();
        let maenads = GodName::Maenads.to_power();

        let next_states = maenads.get_all_next_states(&state);

        for s in next_states {
            if s.get_winner() == Some(Player::One) {
                return;
            }
        }

        assert!(false, "Expected a win, but didn't find one");
    }
}
