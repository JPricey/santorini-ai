use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    direction::{Direction, squares_to_direction},
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            GeneratorPreludeState, build_scored_move, get_generator_prelude_state,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
    utils::hash_u64,
};

use super::PartialAction;

const NUM_BITS_FOR_BUILD_SECTION: usize = 20;
const MOVE_FROM_POSITION_OFFSET: usize = NUM_BITS_FOR_BUILD_SECTION;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct MorpheusMove(pub MoveData);

impl GodMove for MorpheusMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        let mut action = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![action];
        }

        let mut bits = self.0;
        let mut current_dir_idx = 0;
        let mut consecutive = 0;
        for _ in 0..20 {
            if (bits & 1) != 0 {
                let delta = match current_dir_idx {
                    0 => -6,
                    1 => -5,
                    2 => -4,
                    3 => 1,
                    4 => 6,
                    5 => 5,
                    6 => 4,
                    7 => -1,
                    _ => unreachable!(),
                };
                let build_pos = Square::from((self.move_to_position() as i8 + delta as i8) as u8);
                action.push(PartialAction::Build(build_pos));

                consecutive += 1;
                if consecutive == 4 {
                    consecutive = 0;
                    current_dir_idx += 1;
                    if current_dir_idx >= 8 {
                        break;
                    }
                }
            } else {
                consecutive = 0;
                current_dir_idx += 1;
                if current_dir_idx >= 8 {
                    break;
                }
            }
            bits >>= 1;
        }

        vec![action]
    }

    fn make_move(self, board: &mut BoardState, player: Player) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let old_avaiable_builds = board.god_data[player as usize];

        let mut build_bits = self.0 & (1 << NUM_BITS_FOR_BUILD_SECTION) - 1;
        if build_bits.count_ones() == 0 {
            board.set_god_data(player, old_avaiable_builds + 1);
            // No builds
        } else if build_bits.count_ones() == 1 {
            // single build at the direction of the bit index
            let build_dir_idx = build_bits.trailing_zeros();
            let delta = match build_dir_idx {
                0 => -6,
                1 => -5,
                2 => -4,
                3 => 1,
                4 => 6,
                5 => 5,
                6 => 4,
                7 => -1,
                _ => unreachable!(),
            };
            let build_pos = Square::from((self.move_to_position() as i8 + delta as i8) as u8);
            board.build_up(build_pos);
            // Don't change build count
        } else {
            let mut new_available_builds = old_avaiable_builds + 1;
            let mut current_dir_idx = 0;

            while build_bits > 0 {
                let num_zeros = build_bits.trailing_zeros();
                current_dir_idx += num_zeros;
                build_bits >>= num_zeros;

                let mut num_ones = build_bits.trailing_ones();
                build_bits >>= num_ones;

                while num_ones > 0 {
                    let delta = match current_dir_idx {
                        0 => -6,
                        1 => -5,
                        2 => -4,
                        3 => 1,
                        4 => 6,
                        5 => 5,
                        6 => 4,
                        7 => -1,
                        _ => {
                            unreachable!()
                        }
                    };
                    let build_pos =
                        Square::from((self.move_to_position() as i8 + delta as i8) as u8);
                    let build_amt = num_ones.min(4);
                    for _ in 0..build_amt {
                        board.build_up(build_pos);
                    }
                    new_available_builds -= build_amt;

                    if build_amt < 4 {
                        break;
                    } else {
                        current_dir_idx += 1;
                        num_ones -= build_amt;
                    }
                }
            }
            board.set_god_data(player, new_available_builds);
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        let current_res = helper.get();

        let build_mask = self.0 & ((1 << NUM_BITS_FOR_BUILD_SECTION) - 1);
        hash_u64(current_res) ^ hash_u64(build_mask as usize)
    }
}

impl Into<GenericMove> for MorpheusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for MorpheusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl MorpheusMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_counts: &[u8; 8],
    ) -> Self {
        let mut data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET);

        let mut bit_idx = 0;
        for i in 0..8 {
            let current_build_count = build_counts[i];
            if current_build_count == 0 {
                bit_idx += 1;
            } else if current_build_count == 4 {
                data |= 0b1111 << bit_idx;
                bit_idx += 4;
            } else {
                data |= ((1 << current_build_count) - 1) << bit_idx;
                bit_idx += current_build_count + 1;
            }
        }

        Self(data)
    }

    fn new_zero_build_turn(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET);

        Self(data)
    }

    fn new_one_build_turn(
        move_from_position: Square,
        move_to_position: Square,
        build_dir: Direction,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | 1 << (build_dir as usize);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 >> MOVE_FROM_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for MorpheusMove {
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
            let mut result = format!("{}>{}", move_from, move_to);

            let mut bits = self.0;
            let mut current_dir_idx = 0;
            let mut consecutive = 0;
            for _ in 0..20 {
                if (bits & 1) != 0 {
                    let delta = match current_dir_idx {
                        0 => -6,
                        1 => -5,
                        2 => -4,
                        3 => 1,
                        4 => 6,
                        5 => 5,
                        6 => 4,
                        7 => -1,
                        _ => unreachable!(),
                    };
                    let build_pos =
                        Square::from((self.move_to_position() as i8 + delta as i8) as u8);
                    result += &format!("^{}", build_pos);
                    consecutive += 1;
                    if consecutive == 4 {
                        consecutive = 0;
                        current_dir_idx += 1;
                        if current_dir_idx >= 8 {
                            break;
                        }
                    }
                } else {
                    consecutive = 0;
                    current_dir_idx += 1;
                    if current_dir_idx >= 8 {
                        break;
                    }
                }
                bits >>= 1;
            }

            write!(f, "{}", result)
        }
    }
}

fn generate_morpheus_builds<const F: MoveGenFlags>(
    board: &BoardState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    is_improving: bool,
    build_counts: &mut [u8; 8],
    remaining_build_count: usize,
    mut possible_builds: BitBoard,
    blockable_wins: BitBoard,
    mut is_check_already: bool,
    reach_board: BitBoard,
    is_key_squares_resolved: bool,
    key_squares: BitBoard,
) {
    if remaining_build_count == 0 || possible_builds.is_empty() {
        if is_interact_with_key_squares::<F>() && !is_key_squares_resolved {
            return;
        }

        let new_action = MorpheusMove::new_basic_move(start_pos, end_pos, &build_counts);

        let is_check = is_check_already || (possible_builds & blockable_wins).is_not_empty();
        result.push(build_scored_move::<F, _>(
            new_action,
            is_check,
            is_improving,
        ));
        return;
    }

    if remaining_build_count == 1 {
        if is_interact_with_key_squares::<F>() && !is_key_squares_resolved {
            let old = possible_builds;
            possible_builds &= key_squares;

            is_check_already =
                is_check_already || ((possible_builds ^ old) & blockable_wins).is_not_empty();
        }

        if !is_interact_with_key_squares::<F>() || is_key_squares_resolved {
            let new_action = MorpheusMove::new_basic_move(start_pos, end_pos, &build_counts);

            let is_check = is_check_already || (possible_builds & blockable_wins).is_not_empty();
            result.push(build_scored_move::<F, _>(
                new_action,
                is_check,
                is_improving,
            ));
        }

        for build_pos in possible_builds {
            let build_mask = BitBoard::as_mask(build_pos);

            let direction = squares_to_direction(end_pos, build_pos);
            let direction_idx = direction as usize;

            build_counts[direction_idx] = 1;

            let this_build_starting_height = board.get_height(build_pos);
            let is_on_reach_board = (build_mask & reach_board).is_not_empty();
            let this_build_is_check = is_check_already
                || (blockable_wins & possible_builds & !build_mask).is_not_empty()
                || is_on_reach_board && this_build_starting_height == 2;

            let new_action = MorpheusMove::new_basic_move(start_pos, end_pos, &build_counts);

            result.push(build_scored_move::<F, _>(
                new_action,
                this_build_is_check,
                is_improving,
            ));

            build_counts[direction_idx] = 0;
        }
        return;
    }

    let next_build_square = possible_builds.lsb();
    let next_build_mask = BitBoard(possible_builds.0 - 1) & possible_builds;

    let this_build_starting_height = board.get_height(next_build_square);
    let max_builds = (4 - this_build_starting_height).min(remaining_build_count);

    let direction = squares_to_direction(end_pos, next_build_square);
    let direction_idx = direction as usize;

    let this_build_mask = BitBoard::as_mask(next_build_square);
    let is_on_reach_board = (this_build_mask & reach_board).is_not_empty();

    for b in 0..=(max_builds) {
        build_counts[direction_idx] = b as u8;

        let next_is_check = if is_on_reach_board {
            is_check_already || this_build_starting_height + b == 3
        } else {
            is_check_already
        };

        generate_morpheus_builds::<F>(
            board,
            result,
            start_pos,
            end_pos,
            is_improving,
            build_counts,
            remaining_build_count - b,
            next_build_mask,
            blockable_wins,
            next_is_check,
            reach_board,
            is_key_squares_resolved || b >= 1 && (key_squares & this_build_mask).is_not_empty(),
            key_squares,
        );
    }

    build_counts[direction_idx] = 0;
}

fn morpheus_generate_single_build_turns<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    is_improving: bool,
    mut possible_builds: BitBoard,
    reach_board: BitBoard,
    is_key_squares_resolved: bool,
    key_squares: BitBoard,
) {
    // No builds
    if !is_interact_with_key_squares::<F>() || is_key_squares_resolved {
        let zero_build_action = MorpheusMove::new_zero_build_turn(start_pos, end_pos);

        result.push(build_scored_move::<F, _>(
            zero_build_action,
            (reach_board & prelude.exactly_level_3).is_not_empty(),
            is_improving,
        ));
    }

    if is_interact_with_key_squares::<F>() && !is_key_squares_resolved {
        possible_builds &= key_squares;
    }

    for build_pos in possible_builds {
        let build_mask = BitBoard::as_mask(build_pos);

        let direction = squares_to_direction(end_pos, build_pos);

        let final_lvl_3 =
            (prelude.exactly_level_2 & build_mask) | (prelude.exactly_level_3 & !build_mask);
        let is_check = (final_lvl_3 & reach_board).is_not_empty();

        let new_action = MorpheusMove::new_one_build_turn(start_pos, end_pos, direction);

        result.push(build_scored_move::<F, _>(
            new_action,
            is_check,
            is_improving,
        ));
    }
}

pub(super) fn morpheus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(morpheus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let available_builds = state.board.god_data[player as usize] + 1;
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
            if push_winning_moves::<F, MorpheusMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                MorpheusMove::new_winning_move,
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

            if is_interact_with_key_squares::<F>()
                && worker_next_build_state.narrowed_builds.is_empty()
            {
                continue;
            }

            if available_builds == 1 {
                morpheus_generate_single_build_turns::<F>(
                    &prelude,
                    &mut result,
                    worker_start_state.worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.is_improving,
                    worker_next_build_state.all_possible_builds,
                    reach_board,
                    (key_squares & worker_end_move_state.worker_end_mask).is_not_empty(),
                    key_squares,
                )
            } else {
                let mut spent_builds_arr = [0_u8; 8];
                let already_wins = reach_board & prelude.exactly_level_3;
                let is_check_already =
                    (already_wins & !worker_next_build_state.all_possible_builds).is_not_empty();
                let blockable_wins = already_wins & worker_next_build_state.all_possible_builds;

                generate_morpheus_builds::<F>(
                    prelude.board,
                    &mut result,
                    worker_start_state.worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.is_improving,
                    &mut spent_builds_arr,
                    (available_builds as usize).min(12),
                    worker_next_build_state.all_possible_builds,
                    blockable_wins,
                    is_check_already,
                    reach_board,
                    (key_squares & worker_end_move_state.worker_end_mask).is_not_empty(),
                    key_squares,
                );
            }
        }
    }

    result
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(0),
        x => {
            let build_count: u32 = x.parse().map_err(|e| format!("{:?}", e))?;
            Ok(build_count)
        }
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        x => Some(format!("{x}")),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    if board.workers[player as usize].is_empty() {
        return None;
    }

    let mut god_data = board.god_data[player as usize];
    if board.current_player == player {
        god_data += 1;
    }
    Some(format!("Builds tokens: {god_data}"))
}

pub const fn build_morpheus() -> GodPower {
    god_power(
        GodName::Morpheus,
        build_god_power_movers!(morpheus_move_gen),
        build_god_power_actions::<MorpheusMove>(),
        838429420552497011,
        482189877001639000,
    )
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}

#[cfg(test)]
mod tests {
    use crate::{
        fen::parse_fen,
        gods::{GodName, morpheus::MorpheusMove},
        square::Square,
    };

    #[test]
    fn test_basic_morpheus_move() {
        let mut state =
            parse_fen("00000 00000 00000 00000 00000/1/morpheus[1]:B2/mortal:E5").unwrap();
        state.print_to_console();

        let morpheus_action =
            MorpheusMove::new_basic_move(Square::B2, Square::C3, &[0, 0, 0, 0, 1, 0, 1, 0]);

        let morpheus = GodName::Morpheus.to_power();

        morpheus.make_move(&mut state.board, morpheus_action.into());

        state.print_to_console();
    }
}
