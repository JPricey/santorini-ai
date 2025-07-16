use colored::Colorize;

use crate::{
    bitboard::BitBoard,
    fen::{game_state_to_fen, parse_fen},
    gods::{GameStateWithAction, GodName, GodPower},
    player::Player,
    square::Square,
};

use serde::{Deserialize, Serialize};

// const NUM_LEVELS: usize = 4;
pub const BOARD_WIDTH: usize = 5;
pub const NUM_SQUARES: usize = BOARD_WIDTH * BOARD_WIDTH;

pub const IS_WINNER_MASK: BitBoard = BitBoard(1 << 31);
pub const ANTI_WINNER_MASK: BitBoard = BitBoard(!(1 << 31));

pub const NEIGHBOR_MAP: [BitBoard; NUM_SQUARES] = [
    BitBoard(98),
    BitBoard(229),
    BitBoard(458),
    BitBoard(916),
    BitBoard(776),
    BitBoard(3139),
    BitBoard(7335),
    BitBoard(14670),
    BitBoard(29340),
    BitBoard(24856),
    BitBoard(100448),
    BitBoard(234720),
    BitBoard(469440),
    BitBoard(938880),
    BitBoard(795392),
    BitBoard(3214336),
    BitBoard(7511040),
    BitBoard(15022080),
    BitBoard(30044160),
    BitBoard(25452544),
    BitBoard(2195456),
    BitBoard(5472256),
    BitBoard(10944512),
    BitBoard(21889024),
    BitBoard(9175040),
];

#[derive(Clone, PartialEq, Eq)]
pub struct FullGameState {
    pub gods: [&'static GodPower; 2],
    pub board: BoardState,
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
    pub fn new(
        board_state: BoardState,
        p1_god: &'static GodPower,
        p2_god: &'static GodPower,
    ) -> Self {
        FullGameState {
            gods: [p1_god, p2_god],
            board: board_state,
        }
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

    pub fn get_next_states(&self) -> Vec<FullGameState> {
        let active_god = self.get_active_god();
        let board_states_with_action_list = active_god.get_all_next_states(&self.board);
        board_states_with_action_list
            .into_iter()
            .map(|e| FullGameState::new(e, self.gods[0], self.gods[1]))
            .collect()
    }

    pub fn get_next_states_interactive(&self) -> Vec<GameStateWithAction> {
        let active_god = self.get_active_god();
        let board_states_with_action_list = active_god.get_next_states_interactive(&self.board);

        board_states_with_action_list
            .into_iter()
            .map(|e| GameStateWithAction::new(e, self.gods[0].god_name, self.gods[1].god_name))
            .collect()
    }

    pub fn get_god_for_player(&self, player: Player) -> &'static GodPower {
        self.gods[player as usize]
    }

    pub fn get_active_god(&self) -> &'static GodPower {
        self.get_god_for_player(self.board.current_player)
    }

    pub fn get_other_god(&self) -> &'static GodPower {
        self.get_god_for_player(!self.board.current_player)
    }

    pub fn print_to_console(&self) {
        eprintln!("{:?}", self);
        self.board.print_to_console();
    }
}

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
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct BoardState {
    pub current_player: Player,
    // height_map[L - 1][s] represents if square s is GTE L
    pub height_map: [BitBoard; 4],
    pub workers: [BitBoard; 2],
}

impl BoardState {
    pub fn new_basic_state() -> Self {
        let mut result = Self::default();
        result.workers[1].0 |= 1 << 7;
        result.workers[1].0 |= 1 << 17;
        result.workers[0].0 |= 1 << 11;
        result.workers[0].0 |= 1 << 13;
        result
    }

    pub fn flip_current_player(&mut self) {
        self.current_player = !self.current_player;
    }

    pub fn get_height_for_worker(&self, worker_mask: BitBoard) -> usize {
        (
            (self.height_map[0] & worker_mask).0 << 0
                | (self.height_map[1] & worker_mask).0 << 1
                | (self.height_map[2] & worker_mask).0 << 2
            // Worker can't be on dome height, so don't bother checking it
            // | (self.height_map[3] & worker_mask) << 3
        )
        .count_ones() as usize
    }

    pub fn get_true_height(&self, position_mask: BitBoard) -> usize {
        ((self.height_map[0] & position_mask).0 << 0
            | (self.height_map[1] & position_mask).0 << 1
            | (self.height_map[2] & position_mask).0 << 2
            | (self.height_map[3] & position_mask).0 << 3)
            .count_ones() as usize
    }

    pub fn get_winner(&self) -> Option<Player> {
        if (self.workers[0] & IS_WINNER_MASK).0 > 0 {
            Some(Player::One)
        } else if (self.workers[1] & IS_WINNER_MASK).0 > 0 {
            Some(Player::Two)
        } else {
            None
        }
    }

    pub fn set_winner(&mut self, player: Player) {
        let player_idx = player as usize;
        self.workers[player_idx] |= IS_WINNER_MASK;
    }

    pub fn unset_winner(&mut self, player: Player) {
        let player_idx = player as usize;
        self.workers[player_idx] &= ANTI_WINNER_MASK;
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

    pub fn print_for_debugging(&self) {
        for h in 0..4 {
            eprintln!("{h}: {}", self.height_map[h]);
        }
    }

    pub fn confirm_valid(&self) -> Option<usize> {
        for h in 1..4 {
            let height = self.height_map[h];
            let lower = self.height_map[h - 1];

            if (height & !lower).is_not_empty() {
                return Some(h);
            }
        }

        return None;
    }

    pub fn validate_heights(&self) {
        for h in 1..4 {
            let height = self.height_map[h];
            let lower = self.height_map[h - 1];

            if (height & !lower).is_not_empty() {
                for h in 0..4 {
                    eprintln!("{h}: {}", self.height_map[h]);
                }

                panic!("Board has corrupted state on height {h}");
            }
        }
    }

    pub fn print_to_console(&self) {
        if let Some(winner) = self.get_winner() {
            eprintln!("Player {:?} wins!", winner);
        } else {
            eprintln!("Player {:?} to play", self.current_player);
        }

        for row in 0..5 {
            let mut row_str = format!("{}", 5 - row);
            for col in 0..5 {
                let pos = col + row * 5;
                let mask = 1 << pos;
                let height = self.get_true_height(BitBoard(1 << pos));

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

    pub fn get_all_permutations<const INCLUDE_SELF: bool>(&self) -> Vec<Self> {
        let horz = self._flip_horz_clone();
        let vert = self._flip_vertical_clone();
        let hv = horz._flip_vertical_clone();
        let trans = self._transpose_clone();
        let th = trans._flip_horz_clone();
        let tv = trans._flip_vertical_clone();
        let tvh = th._flip_vertical_clone();

        if INCLUDE_SELF {
            vec![self.clone(), horz, vert, hv, trans, th, tv, tvh]
        } else {
            vec![horz, vert, hv, trans, th, tv, tvh]
        }
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

            println!("{board}");
            println!("{flipped}");
        }
    }
}
