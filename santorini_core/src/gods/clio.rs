use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_next_move_state,
            get_worker_start_move_state, is_mate_only, modify_prelude_for_checking_workers,
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
const IS_PLACING_COIN_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const IS_OVER_COIN_OFFSET: usize = IS_PLACING_COIN_OFFSET + POSITION_WIDTH;

const GOD_DATA_COIN_COUNT_OFFSET: usize = 25;
const USED_COIN_MASK: GodData = 0b11 << GOD_DATA_COIN_COUNT_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
struct ClioMove(pub MoveData);

impl GodMove for ClioMove {
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

        let build_pos = self.build_position();
        board.build_up(build_pos);

        let is_placing_coin = self.is_placing_coin();
        let is_over_coin = self.is_over_coin();
        let mut data_diff = ((is_over_coin ^ is_placing_coin) as GodData) << (build_pos as GodData);

        if is_placing_coin {
            let current_used_coins = board.god_data[player as usize] >> GOD_DATA_COIN_COUNT_OFFSET;
            let new_used_coins = current_used_coins + 1;
            let used_coin_delta = current_used_coins ^ new_used_coins;

            data_diff |= used_coin_delta << GOD_DATA_COIN_COUNT_OFFSET;
        }
        board.delta_god_data(player, data_diff);
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

impl Into<GenericMove> for ClioMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ClioMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl ClioMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        is_placing_coin: bool,
        is_over_coin: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((is_placing_coin as MoveData) << IS_PLACING_COIN_OFFSET)
            | ((is_over_coin as MoveData) << IS_OVER_COIN_OFFSET);

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

    fn is_placing_coin(&self) -> bool {
        ((self.0 >> IS_PLACING_COIN_OFFSET) & 1) != 0
    }

    fn is_over_coin(&self) -> bool {
        ((self.0 >> IS_OVER_COIN_OFFSET) & 1) != 0
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for ClioMove {
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
        } else if self.is_placing_coin() {
            write!(f, "{}>{}^{}+", move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

fn clio_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(clio_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let god_data = state.board.god_data[player as usize];
    let coin_mask = BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0);
    let used_coins = god_data >> GOD_DATA_COIN_COUNT_OFFSET;

    let is_placing_coin = used_coins < 3;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, ClioMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                ClioMove::new_winning_move,
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

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let is_over_coin = coin_mask.contains_square(worker_build_pos);

                let new_action = ClioMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                    is_placing_coin,
                    is_over_coin,
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

    let splits = data.split('|').collect::<Vec<_>>();
    if splits.len() != 2 {
        return Err("Clio data must be <used coins>|<comma separated coin squares>".to_string());
    }

    let remaining_coins = splits[0].parse::<u32>().map_err(|e| {
        format!(
            "Failed to parse used coins '{}': {}",
            splits[0],
            e.to_string()
        )
    })?;

    if remaining_coins > 3 {
        return Err("Clio can only use up to 3 coins".to_string());
    }
    let used_coins = 3 - remaining_coins;

    let mut res = BitBoard::EMPTY;
    if !splits[1].trim().is_empty() {
        for part in splits[1].split(',') {
            let square: Square = part
                .trim()
                .parse()
                .map_err(|e| format!("Failed to parse square {}: {:?}", part, e))?;
            res |= BitBoard::as_mask(square);
        }
    }

    let god_data = (res.0 | (used_coins << GOD_DATA_COIN_COUNT_OFFSET)) as GodData;
    Ok(god_data)
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        x => {
            let used_coins = x >> 25;
            let mask_section = BitBoard(x & BitBoard::MAIN_SECTION_MASK.0);

            Some(format!(
                "{}|{}",
                3 - used_coins,
                mask_section
                    .all_squares()
                    .iter()
                    .map(Square::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ))
        }
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => None,
        x => {
            let used_coins = x >> 25;
            let mask_section = BitBoard(x & BitBoard::MAIN_SECTION_MASK.0);

            Some(format!(
                "{} Coins left. Coins at {}.",
                3 - used_coins,
                mask_section
                    .all_squares()
                    .iter()
                    .map(Square::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
}

fn get_frozen_mask(board: &BoardState, player: Player) -> BitBoard {
    BitBoard(board.god_data[player as usize] & BitBoard::MAIN_SECTION_MASK.0)
}

fn flip_horizontal(god_data: GodData) -> GodData {
    BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0)
        .flip_horizontal()
        .0
        | (god_data & USED_COIN_MASK) as GodData
}

fn flip_vertical(god_data: GodData) -> GodData {
    BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0)
        .flip_vertical()
        .0
        | (god_data & USED_COIN_MASK) as GodData
}

fn flip_transpose(god_data: GodData) -> GodData {
    BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0)
        .flip_transpose()
        .0
        | (god_data & USED_COIN_MASK) as GodData
}

pub const fn build_clio() -> GodPower {
    god_power(
        GodName::Clio,
        build_god_power_movers!(clio_move_gen),
        build_god_power_actions::<ClioMove>(),
        4755690011371988784,
        3211938079590198314,
    )
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
    .with_get_frozen_mask_fn(get_frozen_mask)
    .with_flip_god_data_horizontal_fn(flip_horizontal)
    .with_flip_god_data_vertical_fn(flip_vertical)
    .with_flip_god_data_transpose_fn(flip_transpose)
}
