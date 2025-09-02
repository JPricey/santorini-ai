use core::panic;

use colored::Colorize;
use const_for::const_for;
use strum::IntoEnumIterator;

use crate::{
    bitboard::BitBoard,
    direction::ICoord,
    fen::{game_state_to_fen, parse_fen},
    gods::{
        BoardStateWithAction, GameStateWithAction, GodName, StaticGod,
        generic::{GenericMove, GodMove},
    },
    hashing::{
        HashType, ZORBRIST_HEIGHT_RANDOMS, ZORBRIST_PLAYER_TWO, ZORBRIST_WORKER_RANDOMS,
        compute_hash_from_scratch_for_board,
    },
    placement::{get_all_placements, get_all_placements_3, get_starting_placements_count},
    player::Player,
    square::Square,
    transmute_enum,
};

use serde::{Deserialize, Serialize};

pub const NUM_LEVELS: usize = 4;
pub const BOARD_WIDTH: usize = 5;
pub const NUM_SQUARES: usize = BOARD_WIDTH * BOARD_WIDTH;

#[macro_export]
macro_rules! for_each_direction {
    ($dir: ident => $body: block) => {
        use const_for::const_for;
        const_for!(i in 0..8 => {
            let $dir = $crate::direction::Direction::from_u8(i);
            $body
        })
    }
}

#[macro_export]
macro_rules! square_map {
    ($square: ident => $body: expr) => {{
        let mut arr: [core::mem::MaybeUninit<_>; NUM_SQUARES] =
            unsafe { core::mem::MaybeUninit::uninit().assume_init() };
        let mut i = 0;
        while i < NUM_SQUARES {
            let $square: Square = $crate::transmute_enum!(i as u8);
            arr[i] = core::mem::MaybeUninit::new($body);
            i += 1;
        }
        unsafe { std::mem::transmute_copy::<_, [_; NUM_SQUARES]>(&arr) }
    }};
}

pub const NEIGHBOR_MAP: [BitBoard; NUM_SQUARES] = square_map!(square => {
    let mut res = BitBoard::EMPTY;
    let coord = square.to_icoord();
    for_each_direction!(dir => {
        let new_coord = coord.add(dir.to_icoord());
        if let Some(n) = new_coord.to_square() {
            res = res.bit_or(BitBoard::as_mask(n));
        }
    });
    res
});

pub const INCLUSIVE_NEIGHBOR_MAP: [BitBoard; NUM_SQUARES] = square_map!(square => {
    let coord = square.to_icoord();
    let mut res = BitBoard::as_mask(square);
    for_each_direction!(dir => {
        let new_coord = coord.add(dir.to_icoord());
        if let Some(n) = new_coord.to_square() {
            res = res.bit_or(BitBoard::as_mask(n));
        }
    });
    res
});

pub const WRAPPING_NEIGHBOR_MAP: [BitBoard; NUM_SQUARES] = square_map!(square => {
    let coord = square.to_icoord();
    let mut res = BitBoard::as_mask(square);
    for_each_direction!(dir => {
        let mut new_coord = coord.add(dir.to_icoord()).add(ICoord::new(5, 5));
        new_coord.col %= 5;
        new_coord.row %= 5;

        res = res.bit_or(BitBoard::as_mask(new_coord.to_square().unwrap()));
    });
    res
});

pub const PUSH_MAPPING: [[Option<Square>; NUM_SQUARES]; NUM_SQUARES] = {
    let mut result = [[None; NUM_SQUARES]; NUM_SQUARES];
    const_for!(from in 0..25 => {
        const_for!(to in 0..25 => {
            let to_mask = BitBoard::as_mask(transmute_enum!(to as u8));
            if (NEIGHBOR_MAP[from as usize].0 & to_mask.0) != 0 {
                let delta = to - from;
                let dest = to + delta;
                if dest >= 0 && dest < 25 {
                    if NEIGHBOR_MAP[to as usize].0 & 1 << dest != 0 {
                        result[from as usize][to as usize] = Some(transmute_enum!(dest as u8));
                    }
                }
            }
        });
    });
    result
};

pub const MIDDLE_SPACES_MASK: BitBoard = BitBoard(0b00000_01110_01110_01110_00000);
pub const PERIMETER_SPACES_MASK: BitBoard = MIDDLE_SPACES_MASK
    .bit_not()
    .bit_and(BitBoard::MAIN_SECTION_MASK);

#[derive(Clone, PartialEq, Eq)]
pub struct FullGameState {
    pub board: BoardState,
    pub gods: [StaticGod; 2],
}

impl Serialize for FullGameState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let fen = game_state_to_fen(self);
        serializer.serialize_str(&fen)
    }
}

impl<'de> Deserialize<'de> for FullGameState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fen: String = Deserialize::deserialize(deserializer)?;
        parse_fen(&fen).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for FullGameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", game_state_to_fen(self))
    }
}

impl std::fmt::Debug for FullGameState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl TryFrom<&str> for FullGameState {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}

impl TryFrom<&String> for FullGameState {
    type Error = String;

    fn try_from(s: &String) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}

impl FullGameState {
    pub fn new(board_state: BoardState, p1_god: StaticGod, p2_god: StaticGod) -> Self {
        let mut res = FullGameState {
            gods: [p1_god, p2_god],
            board: board_state,
        };
        res.recalculate_internals();
        res
    }

    pub fn new_empty_state(p1: GodName, p2: GodName) -> Self {
        FullGameState::new(BoardState::default(), p1.to_power(), p2.to_power())
    }

    pub fn new_basic_state(p1: GodName, p2: GodName) -> Self {
        FullGameState::new(BoardState::new_basic_state(), p1.to_power(), p2.to_power())
    }

    pub fn new_basic_state_mortals() -> Self {
        FullGameState::new_basic_state(GodName::Mortal, GodName::Mortal)
    }

    pub fn flip_current_player(&mut self) {
        self.board.flip_current_player();
    }

    pub fn next_state(&self, god: StaticGod, action: GenericMove) -> FullGameState {
        let mut result = self.clone();
        god.make_move(&mut result.board, action);
        result
    }

    pub fn get_next_states(&self) -> Vec<FullGameState> {
        let active_god = self.get_active_god();
        let board_states_with_action_list = active_god.get_all_next_states(&self);
        board_states_with_action_list
            .into_iter()
            .map(|e| FullGameState::new(e, self.gods[0], self.gods[1]))
            .collect()
    }

    pub fn get_next_states_interactive(&self) -> Vec<GameStateWithAction> {
        let placement_mode = get_starting_placements_count(&self.board).unwrap();
        if placement_mode > 0 {
            let active_god = self.get_active_god();
            let placement_actions = match active_god.num_workers {
                2 => get_all_placements(&self.board),
                3 => get_all_placements_3(&self.board),
                _ => unreachable!("Unknown number of workers"),
            };
            let mut res: Vec<GameStateWithAction> = Vec::new();

            for p in placement_actions {
                for series in p.move_to_actions(&self.board) {
                    let mut new_board = self.board.clone();
                    p.make_move(&mut new_board);

                    let board_state_w_action = BoardStateWithAction::new(new_board, series);

                    res.push(GameStateWithAction::new(
                        board_state_w_action,
                        self.gods[0].god_name,
                        self.gods[1].god_name,
                    ));
                }
            }

            res
        } else {
            let active_god = self.get_active_god();
            let board_states_with_action_list = active_god.get_next_states_interactive(&self);

            board_states_with_action_list
                .into_iter()
                .map(|e| GameStateWithAction::new(e, self.gods[0].god_name, self.gods[1].god_name))
                .collect()
        }
    }

    pub fn get_active_non_active_gods(&self) -> (StaticGod, StaticGod) {
        match self.board.current_player {
            Player::One => (self.gods[0], self.gods[1]),
            Player::Two => (self.gods[1], self.gods[0]),
        }
    }

    pub fn get_god_for_player(&self, player: Player) -> StaticGod {
        self.gods[player as usize]
    }

    pub fn get_active_god(&self) -> StaticGod {
        self.get_god_for_player(self.board.current_player)
    }

    pub fn get_other_god(&self) -> StaticGod {
        self.get_god_for_player(!self.board.current_player)
    }

    pub fn print_to_console(&self) {
        eprintln!("{:?}", self);
        self.board.print_to_console();
    }

    pub fn base_hash(&self) -> HashType {
        self.gods[0].hash1 ^ self.gods[1].hash2
    }

    pub fn recalculate_internals(&mut self) {
        self.board.recalculate_internals(self.base_hash());
    }

    pub fn validation_err(&self) -> Result<(), String> {
        self.board.validation_err(self.base_hash(), self.gods)
    }

    pub fn validate(&self) {
        self.validation_err().unwrap();
    }

    pub fn get_winner(&self) -> Option<Player> {
        self.board.get_winner()
    }
}

pub const WINNER_SIGNAL_HEIGHT_BOARD_INDEX: usize = 0;
pub const WINNER_MASK_OFFSET: usize = 30;
pub const IS_WINNER_MASK: BitBoard = BitBoard(0b11 << WINNER_MASK_OFFSET);
pub const P1_WINNER: BitBoard = BitBoard(0b01 << WINNER_MASK_OFFSET);
pub const P2_WINNER: BitBoard = BitBoard(0b10 << WINNER_MASK_OFFSET);
pub const ANTI_WINNER_MASK: BitBoard = BitBoard(!(0b11 << WINNER_MASK_OFFSET));
pub const WINNER_LOOKUP: [Option<Player>; 3] = [None, Some(Player::One), Some(Player::Two)];

pub const PLAYER_TO_WINNER_LOOKUP: [BitBoard; 2] = [P1_WINNER, P2_WINNER];

pub const HEIGHT_RESTRICTION_HEIGHT_BOARD_INDEX: usize = 1;
pub const HEIGHT_RESTRICTION_BASE_OFFSET: usize = 30;
pub const HEIGHT_RESTRICTION_P1_MASK: BitBoard = P1_WINNER;
pub const HEIGHT_RESTRICTION_P2_MASK: BitBoard = P2_WINNER;
pub const HEIGHT_RESTRICTION_MASK_BY_PLAYER: [BitBoard; 2] =
    [HEIGHT_RESTRICTION_P1_MASK, HEIGHT_RESTRICTION_P2_MASK];
pub const HEIGHT_RESTRICTION_SECTION_MASK: BitBoard = BitBoard(0b11 << 30);

/*
 * Bitmap of each level:
 * 0  1  2  3  4
 * 5  6  7  8  9
 * 10 11 12 13 14
 * 15 16 17 18 19
 * 20 21 22 23 24
 *
 * 25 26 27 28 29
 * 30 31
 *
 * bits 25-29 are best kept clear to not mess with worker moving helpers
 *  eg: move_all_workers_one_include_original_workers
 *
 * on board 0:
 * 30-31 represent winners
 * 00 for no winner
 * 10 for player 1 winning
 * 01 for player 2 winning
 * 11 is invalid state
 *
 * on board 1:
 * 30-31 represent how much a worker can move up (for athena)
 * bit 30 represents the amount that player 1 can move up on their next turn
 * bit 31 represents the amount that player 2 can move up on their next turn
 * move of the time these will be "11", they only change to 0 when playing against athena
 */
#[derive(Clone, Debug, Default)]
pub struct BoardState {
    pub current_player: Player,
    // height_map[L - 1][s] represents if square s is GTE L
    pub height_map: [BitBoard; 4],
    pub workers: [BitBoard; 2],

    pub hash: HashType,
    height_lookup: [u8; 25],
}

impl PartialEq for BoardState {
    fn eq(&self, other: &Self) -> bool {
        self.current_player == other.current_player
            && self.height_map == other.height_map
            && self.workers == other.workers
    }
}

impl Eq for BoardState {}

impl BoardState {
    pub fn new_basic_state() -> Self {
        let mut result = Self::default();
        result.workers[1].0 |= 1 << 7;
        result.workers[1].0 |= 1 << 17;
        result.workers[0].0 |= 1 << 11;
        result.workers[0].0 |= 1 << 13;
        result
    }

    pub fn recalculate_internals(&mut self, base_hash: HashType) {
        self.hash = compute_hash_from_scratch_for_board(self, base_hash);
        for square in Square::iter() {
            self.height_lookup[square as usize] =
                self._calculate_height(BitBoard::as_mask(square)) as u8;
        }
    }

    pub fn flip_current_player(&mut self) {
        self.current_player = !self.current_player;
        self.hash ^= ZORBRIST_PLAYER_TWO;
    }

    pub fn get_height(&self, position: Square) -> usize {
        self.height_lookup[position as usize] as usize
    }

    fn _calculate_height(&self, position_mask: BitBoard) -> usize {
        ((self.height_map[0] & position_mask).0 << 0
            | (self.height_map[1] & position_mask).0 << 1
            | (self.height_map[2] & position_mask).0 << 2
            | (self.height_map[3] & position_mask).0 << 3)
            .count_ones() as usize
    }

    pub fn get_winner(&self) -> Option<Player> {
        WINNER_LOOKUP[self.height_map[0].0 as usize >> WINNER_MASK_OFFSET]
    }

    pub fn set_winner(&mut self, player: Player) {
        debug_assert_eq!(self.height_map[0].0 as usize >> WINNER_MASK_OFFSET, 0);
        self.height_map[0] ^= PLAYER_TO_WINNER_LOOKUP[player as usize];
        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[0][WINNER_MASK_OFFSET + player as usize];
    }

    pub fn unset_winner(&mut self, player: Player) {
        let player_bit = WINNER_MASK_OFFSET + player as usize;
        debug_assert_eq!(
            self.height_map[0].0 as usize & 1 << player_bit,
            1 << player_bit
        );

        self.height_map[0] ^= PLAYER_TO_WINNER_LOOKUP[player as usize];
        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[0][player_bit];
    }

    pub fn get_worker_can_climb(&self, player: Player) -> bool {
        (self.height_map[HEIGHT_RESTRICTION_HEIGHT_BOARD_INDEX]
            & HEIGHT_RESTRICTION_MASK_BY_PLAYER[player as usize])
            .is_empty()
    }

    pub fn flip_worker_can_climb(&mut self, player: Player, bit: bool) {
        if bit {
            let idx = HEIGHT_RESTRICTION_BASE_OFFSET + (player as usize);
            self.height_map[HEIGHT_RESTRICTION_HEIGHT_BOARD_INDEX] ^= BitBoard(1 << idx);
            self.hash ^= ZORBRIST_HEIGHT_RANDOMS[HEIGHT_RESTRICTION_HEIGHT_BOARD_INDEX][idx];
        }
    }

    pub fn unset_worker_can_climb(&mut self) {
        self.flip_worker_can_climb(Player::One, !self.get_worker_can_climb(Player::One));
        self.flip_worker_can_climb(Player::Two, !self.get_worker_can_climb(Player::Two));
    }

    pub fn get_worker_climb_height(&self, player: Player, current_height: usize) -> usize {
        3.min(current_height + self.get_worker_can_climb(player) as usize)
    }

    pub fn exactly_level_0(&self) -> BitBoard {
        !self.at_least_level_1()
    }

    pub fn exactly_level_1(&self) -> BitBoard {
        self.height_map[0] & !self.height_map[1]
    }

    pub fn exactly_level_2(&self) -> BitBoard {
        self.height_map[1] & !self.height_map[2]
    }

    pub fn exactly_level_3(&self) -> BitBoard {
        self.height_map[2] & !self.height_map[3]
    }

    pub fn exactly_level_n(&self, level: usize) -> BitBoard {
        if level == 0 {
            self.exactly_level_0()
        } else {
            self.height_map[level - 1] & !self.height_map[level]
        }
    }

    pub fn at_least_level_1(&self) -> BitBoard {
        self.height_map[0]
    }

    pub fn at_least_level_2(&self) -> BitBoard {
        self.height_map[1]
    }

    pub fn at_least_level_3(&self) -> BitBoard {
        self.height_map[2]
    }

    pub fn at_least_level_4(&self) -> BitBoard {
        self.height_map[3]
    }

    pub fn at_least_level_n(&self, level: usize) -> BitBoard {
        if level == 0 {
            BitBoard::MAIN_SECTION_MASK
        } else {
            self.height_map[level - 1]
        }
    }

    pub fn worker_xor(&mut self, player: Player, xor: BitBoard) {
        self.workers[player as usize] ^= xor;
        for pos in xor {
            self.hash ^= ZORBRIST_WORKER_RANDOMS[player as usize][pos as usize];
        }
    }

    pub fn build_up(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position);
        self.height_map[current_height] ^= build_mask;
        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];
        self.height_lookup[build_position as usize] += 1;
    }

    pub fn double_build_up(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position);
        self.height_map[current_height] ^= build_mask;
        self.height_map[current_height + 1] ^= build_mask;
        self.height_lookup[build_position as usize] += 2;

        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];
        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[current_height + 1][build_position as usize];
    }

    pub fn dome_up(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position);
        for h in current_height..4 {
            self.height_map[h] ^= build_mask;
            self.hash ^= ZORBRIST_HEIGHT_RANDOMS[h][build_position as usize];
        }
        self.height_lookup[build_position as usize] = 4;
    }

    pub fn unbuild(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position) - 1;
        self.height_map[current_height] ^= build_mask;
        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];
        self.height_lookup[build_position as usize] -= 1;
    }

    pub fn double_unbuild(&mut self, build_position: Square) {
        let build_mask = BitBoard::as_mask(build_position);
        let current_height = self.get_height(build_position) - 1;
        self.height_map[current_height] ^= build_mask;
        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[current_height][build_position as usize];

        self.height_map[current_height - 1] ^= build_mask;
        self.hash ^= ZORBRIST_HEIGHT_RANDOMS[current_height - 1][build_position as usize];

        self.height_lookup[build_position as usize] -= 2;
    }

    pub fn undome(&mut self, build_position: Square, final_height: usize) {
        let build_mask = BitBoard::as_mask(build_position);
        for h in final_height..4 {
            self.height_map[h] ^= build_mask;
            self.hash ^= ZORBRIST_HEIGHT_RANDOMS[h][build_position as usize];
        }
        self.height_lookup[build_position as usize] = final_height as u8;
    }

    pub fn print_for_debugging(&self) {
        for h in 0..4 {
            eprintln!("{h}: {}", self.height_map[h]);
            eprintln!("{:032b}", self.height_map[h].0);
        }
    }

    fn _validate_player(&self, player: Player, god: StaticGod) -> Result<(), String> {
        let player_idx = player as usize;
        let player_workers = self.workers[player_idx];

        let worker_count = player_workers.count_ones();

        if god.god_name == GodName::Hermes {
            if worker_count > 2 {
                return Err("Player {:?} Hrmes can't have more than 2 workers".to_owned());
            }
        } else if worker_count > 4 {
            return Err(format!("Player {:?} has too many workers", player));
        }

        let dome_worker_collide = player_workers & self.height_map[3];
        if dome_worker_collide.is_not_empty() {
            return Err(format!("Player {:?} has workers on domes", player));
        }

        if god.is_hypnus() && self.workers[1 - player_idx].count_ones() == 1 {
            return Err("Can't play hypnus against a solo worker".to_owned());
        }

        Ok(())
    }

    pub fn validation_err(&self, base_hash: HashType, gods: [StaticGod; 2]) -> Result<(), String> {
        let starting_placements = get_starting_placements_count(self)?;
        if starting_placements == 1 {
            if self.current_player != Player::Two {
                return Err("Should be player twos turn to place".to_owned());
            }
        } else if starting_placements == 2 {
            if self.current_player != Player::One {
                return Err("Should be player ones turn to place".to_owned());
            }
        }

        self._validate_player(Player::One, gods[0])?;
        self._validate_player(Player::Two, gods[1])?;

        for h in 1..4 {
            let height = self.height_map[h] & BitBoard::MAIN_SECTION_MASK;
            let lower = self.height_map[h - 1] & BitBoard::MAIN_SECTION_MASK;

            if (height & !lower).is_not_empty() {
                for h in 0..4 {
                    eprintln!("{h}: {}", self.height_map[h]);
                }

                return Err(format!("Board has corrupted state on height {h}"));
            }
        }

        if self.hash != compute_hash_from_scratch_for_board(self, base_hash) {
            let diff = self.hash ^ compute_hash_from_scratch_for_board(self, base_hash);
            return Err(format!(
                "Hash mismatch: expected {:064b}, got {:064b} (diff: {:064b})",
                compute_hash_from_scratch_for_board(self, base_hash),
                self.hash,
                diff
            ));
        }

        Ok(())
    }

    pub fn get_worker_at(&self, square: Square) -> Option<Player> {
        if (self.workers[0] & BitBoard::as_mask(square)).is_not_empty() {
            Some(Player::One)
        } else if (self.workers[1] & BitBoard::as_mask(square)).is_not_empty() {
            Some(Player::Two)
        } else {
            None
        }
    }

    pub fn print_to_console(&self) {
        if let Some(winner) = self.get_winner() {
            eprintln!("Player {:?} wins!", winner);
        } else {
            eprintln!("Player {:?} to play", self.current_player);
        }

        for row in 0_usize..5 {
            let mut row_str = format!("{}", 5 - row);
            for col in 0_usize..5 {
                let pos = col + row * 5;
                let mask = 1 << pos;
                let height = self.get_height(pos.into());

                let is_1 = self.workers[0].0 & mask > 0;
                let is_2 = self.workers[1].0 & mask > 0;

                assert!(
                    !(is_1 && is_2),
                    "A square cannot have both players' workers"
                );

                let char = if is_1 {
                    "X"
                } else if is_2 {
                    "0"
                } else {
                    " "
                }
                .black();

                let elem = match height {
                    0 => char.on_white(),
                    1 => char.on_yellow(),
                    2 => char.on_blue(),
                    3 => char.on_green(),
                    4 => char.on_black(),
                    _ => panic!("Invalid Height: {}", height),
                };
                row_str = format!("{row_str}{elem}");
            }
            eprint!("{}", row_str);
            eprintln!()
        }
        eprintln!(" ABCDE");
    }

    pub fn get_positions_for_player(&self, player: Player) -> Vec<Square> {
        let workers_mask = self.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;
        workers_mask.into_iter().collect()
    }

    pub fn as_basic_game_state(&self) -> FullGameState {
        FullGameState::new(
            self.clone(),
            GodName::Mortal.to_power(),
            GodName::Mortal.to_power(),
        )
    }

    fn _flip_vertical_mut(&mut self) {
        self.height_map[0] = _flip_bitboard_vertical(self.height_map[0]);
        self.height_map[1] = _flip_bitboard_vertical(self.height_map[1]);
        self.height_map[2] = _flip_bitboard_vertical(self.height_map[2]);
        self.height_map[3] = _flip_bitboard_vertical(self.height_map[3]);
        self.workers[0] = _flip_bitboard_vertical(self.workers[0]);
        self.workers[1] = _flip_bitboard_vertical(self.workers[1]);
    }

    fn _flip_horizontal_mut(&mut self) {
        self.height_map[0] = _flip_bitboard_horizontal(self.height_map[0]);
        self.height_map[1] = _flip_bitboard_horizontal(self.height_map[1]);
        self.height_map[2] = _flip_bitboard_horizontal(self.height_map[2]);
        self.height_map[3] = _flip_bitboard_horizontal(self.height_map[3]);
        self.workers[0] = _flip_bitboard_horizontal(self.workers[0]);
        self.workers[1] = _flip_bitboard_horizontal(self.workers[1]);
    }

    fn _transpose_mut(&mut self) {
        self.height_map[0] = _transpose_bitboard(self.height_map[0]);
        self.height_map[1] = _transpose_bitboard(self.height_map[1]);
        self.height_map[2] = _transpose_bitboard(self.height_map[2]);
        self.height_map[3] = _transpose_bitboard(self.height_map[3]);
        self.workers[0] = _transpose_bitboard(self.workers[0]);
        self.workers[1] = _transpose_bitboard(self.workers[1]);
    }

    fn _flip_vertical_clone(&self) -> Self {
        let mut result = self.clone();
        result._flip_vertical_mut();
        result
    }

    fn _flip_horz_clone(&self) -> Self {
        let mut result = self.clone();
        result._flip_horizontal_mut();
        result
    }

    fn _transpose_clone(&self) -> Self {
        let mut result = self.clone();
        result._transpose_mut();
        result
    }

    pub fn get_all_permutations<const INCLUDE_SELF: bool>(&self, base_hash: HashType) -> Vec<Self> {
        let horz = self._flip_horz_clone();
        let vert = self._flip_vertical_clone();
        let hv = horz._flip_vertical_clone();
        let trans = self._transpose_clone();
        let th = trans._flip_horz_clone();
        let tv = trans._flip_vertical_clone();
        let tvh = th._flip_vertical_clone();

        let mut res = if INCLUDE_SELF {
            vec![self.clone(), horz, vert, hv, trans, th, tv, tvh]
        } else {
            vec![horz, vert, hv, trans, th, tv, tvh]
        };

        for board in &mut res {
            board.recalculate_internals(base_hash);
        }

        res
    }

    // Returns a canonically permuted board state
    // WARNING: this is done somewhat inefficiently by actually constructing a list of all permutations and
    // then finding the "smallest". We'll probably want this in the search loop at some point, but maybe not in this form.
    // pub fn get_canonical_permutation(&self) -> Self {
    //     self.get_all_permutations()
    //         .into_iter()
    //         .min_by(|a, b| {
    //             a.height_map[0]
    //                 .cmp(b.height_map[0])
    //                 .then(a.height_map[1].cmp(b.height_map[1]))
    //                 .then(a.height_map[2].cmp(b.height_map[2]))
    //                 .then(a.height_map[3].cmp(b.height_map[3]))
    //                 .then(a.workers[0].cmp(b.workers[0]))
    //                 .then(a.workers[1].cmp(b.workers[1]))
    //         })
    //         .unwrap()
    // }
}

pub fn get_all_permutations_for_pair(
    a: &BoardState,
    b: &BoardState,
) -> Vec<(BoardState, BoardState)> {
    let h = (a._flip_horz_clone(), b._flip_horz_clone());
    let v = (a._flip_vertical_clone(), b._flip_vertical_clone());
    let hv = (h.0._flip_vertical_clone(), h.1._flip_vertical_clone());
    let t = (a._transpose_clone(), b._transpose_clone());
    let th = (t.0._flip_horz_clone(), t.1._flip_horz_clone());
    let tv = (t.0._flip_vertical_clone(), t.1._flip_vertical_clone());
    let thv = (th.0._flip_vertical_clone(), th.1._flip_vertical_clone());

    vec![(a.clone(), b.clone()), h, v, hv, t, th, tv, thv]
}

impl Ord for BoardState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.height_map[0]
            .cmp(other.height_map[0])
            .then(self.height_map[1].cmp(other.height_map[1]))
            .then(self.height_map[2].cmp(other.height_map[2]))
            .then(self.height_map[3].cmp(other.height_map[3]))
            .then(self.workers[0].cmp(other.workers[0]))
            .then(self.workers[1].cmp(other.workers[1]))
    }
}

impl PartialOrd for BoardState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

#[inline]
fn _delta_swap(board: BitBoard, mask: u32, shift: u32) -> BitBoard {
    let delta = ((board.0 >> shift) ^ board.0) & mask;
    BitBoard((board.0 ^ delta) ^ (delta << shift))
}

fn _flip_bitboard_vertical(mut board: BitBoard) -> BitBoard {
    board = _delta_swap(board, 0b11111, 20);
    board = _delta_swap(board, 0b1111100000, 10);
    board
}

fn _flip_bitboard_horizontal(mut board: BitBoard) -> BitBoard {
    board = _delta_swap(board, 0b00001_00001_00001_00001_00001, 4);
    board = _delta_swap(board, 0b00010_00010_00010_00010_00010, 2);
    board
}

fn _transpose_bitboard(mut board: BitBoard) -> BitBoard {
    // https://stackoverflow.com/questions/72097570/rotate-and-reflect-a-5x5-bitboard
    board = _delta_swap(board, 0x00006300, 16);
    board = _delta_swap(board, 0x020a080a, 4);
    board = _delta_swap(board, 0x0063008c, 8);
    board = _delta_swap(board, 0x00006310, 16);
    board
}

#[cfg(test)]
mod tests {
    use crate::square::Square;

    use super::*;

    #[test]
    fn test_serde_coord() {
        for position in 0_usize..25 {
            let coord = Square::from(position);
            let coord_str = serde_json::to_string(&coord).unwrap();
            let parsed_coord: Square = serde_json::from_str(&coord_str).unwrap();

            assert_eq!(coord, parsed_coord);
        }
    }

    #[test]
    fn test_flip_board_v() {
        for b in 0..25 {
            let board: BitBoard = BitBoard(1 << b);
            let row = b / 5;
            let col = b % 5;

            let flipped = _flip_bitboard_vertical(board);
            let pos = flipped.0.trailing_zeros();
            let arow = pos / 5;
            let acol = pos % 5;

            assert_eq!(arow, 4 - row);
            assert_eq!(acol, col);
        }
    }

    #[test]
    fn test_flip_board_h() {
        for b in 0..25 {
            let board = BitBoard(1 << b);
            let row = b / 5;
            let col = b % 5;

            let flipped = _flip_bitboard_horizontal(board);
            let pos = flipped.trailing_zeros();
            let arow = pos / 5;
            let acol = pos % 5;

            assert_eq!(arow, row);
            assert_eq!(acol, 4 - col);
        }
    }

    #[test]
    fn test_transpose() {
        for b in 0..25 {
            let board = BitBoard(1 << b);
            let row = b / 5;
            let col = b % 5;

            let flipped = _transpose_bitboard(board);

            let pos = flipped.trailing_zeros();
            let arow = pos / 5;
            let acol = pos % 5;

            assert_eq!(row, acol);
            assert_eq!(col, arow);

            // println!("{board}");
            // println!("{flipped}");
        }
    }
}
