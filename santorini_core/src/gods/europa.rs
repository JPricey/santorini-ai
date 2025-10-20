use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        build_god_power_actions, generic::{
            GenericMove, GodMove, MoveData, MoveGenFlags, ScoredMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, NULL_MOVE_DATA, POSITION_WIDTH
        }, god_power, move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state_with_custom_worker_helper, get_worker_next_build_state,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        }, FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const TALUS_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) struct EuropaMove(pub MoveData);

impl Into<GenericMove> for EuropaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for EuropaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

// Talus position of "32" is unplaced
impl EuropaMove {
    pub fn new_set_talus_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        talus_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((talus_position as MoveData) << TALUS_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_no_talus_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << TALUS_POSITION_OFFSET);

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

    pub fn talus_position(self) -> Option<Square> {
        let pos = (self.0 >> TALUS_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if pos < 25 {
            Some(Square::from(pos))
        } else {
            None
        }
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for EuropaMove {
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
            if let Some(talus_pos) = self.talus_position() {
                write!(f, "{}>{}^{} T{}", move_from, move_to, build, talus_pos)
            } else {
                write!(f, "{}>{}^{}", move_from, move_to, build)
            }
        }
    }
}

impl GodMove for EuropaMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        if let Some(talus_position) = self.talus_position() {
            res.push(PartialAction::SetTalusPosition(talus_position));
        }

        vec![res]
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

        if let Some(talus_pos) = self.talus_position() {
            board.set_god_data(player, BitBoard::as_mask(talus_pos).0 as GodData);
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
        helper.get()
    }
}

pub(super) fn europa_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(europa_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    // At the start of a match the mask may be zero, so don't convert positions into squares
    let current_talus_mask = BitBoard(state.board.god_data[player as usize]);
    // let current_talus_pos = current_talus_mask.0.trailing_zeros();
    let anti_current_talus_mask = !current_talus_mask;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        worker_next_moves.worker_moves &= anti_current_talus_mask;

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, EuropaMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                EuropaMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if F & super::generic::MATE_ONLY != 0 {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state = get_worker_end_move_state_with_custom_worker_helper::<F>(
                &prelude,
                &worker_start_state,
                worker_end_pos,
                prelude.all_workers_and_frozen_mask | current_talus_mask,
            );
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

            let new_talus_positions = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & worker_next_build_state.unblocked_squares
                & anti_current_talus_mask;

            for new_talus_pos in new_talus_positions {
                let new_talus_mask = BitBoard::as_mask(new_talus_pos);
                let anti_new_talus_mask = !new_talus_mask;

                let mut build_positions = if !is_interact_with_key_squares::<F>()
                    || ((new_talus_mask | worker_end_move_state.worker_end_mask) & key_squares)
                        .is_not_empty()
                {
                    worker_next_build_state.all_possible_builds
                } else {
                    worker_next_build_state.narrowed_builds
                };

                build_positions &= anti_current_talus_mask;
                build_positions &= !(new_talus_mask & prelude.exactly_level_3);

                for worker_build_pos in build_positions & anti_current_talus_mask {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);

                    let new_action = EuropaMove::new_set_talus_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                        new_talus_pos,
                    );
                    let is_check = {
                        let final_level_3 = ((prelude.exactly_level_2 & worker_build_mask)
                            | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos)))
                            & anti_new_talus_mask;
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

            for worker_build_pos in
                worker_next_build_state.narrowed_builds & anti_current_talus_mask
            {
                let new_action = EuropaMove::new_no_talus_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_3
                            & !BitBoard::as_mask(worker_build_pos)
                            & anti_current_talus_mask);
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
        0 => Some("Talus unplaced".to_string()),
        x => Some(format!("Talus at {:?}", BitBoard(x).lsb())),
    }
}

fn get_frozen_mask(board: &BoardState, player: Player) -> BitBoard {
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

pub const fn build_europa() -> GodPower {
    god_power(
        GodName::Europa,
        build_god_power_movers!(europa_move_gen),
        build_god_power_actions::<EuropaMove>(),
        10238480885541372364,
        2504683456410965362,
    )
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
    .with_get_frozen_mask_fn(get_frozen_mask)
    .with_flip_god_data_horizontal_fn(flip_horizontal)
    .with_flip_god_data_vertical_fn(flip_vertical)
    .with_flip_god_data_transpose_fn(flip_transpose)
}

#[cfg(test)]
mod tests {
    use crate::fen::{game_state_to_fen, parse_fen};

    #[test]
    fn test_europa_parse_round_trip() {
        let initial_fen = "0000000000000010000000000/2/europa[E2]:A3,D2/persephone:C4,B3";
        let state = parse_fen(&initial_fen).unwrap();
        let new_fen = game_state_to_fen(&state);
        assert_eq!(initial_fen, new_fen);
    }

    // #[test]
    // fn test_europa_permutations() {
    //     let state =
    //         parse_fen("01000 00200 00000 00000 00000/1/europa[d2]:E1,E2/mortal:A1").unwrap();

    //     for s in state.get_all_permutations::<true>() {
    //         let f = FullGameState::new(s, state.gods);
    //         eprintln!("{:?}", f);
    //         f.print_to_console();
    //     }
    // }
}
