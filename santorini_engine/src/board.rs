use colored::Colorize;

use crate::fen::{board_to_fen, parse_fen};

use super::search::{Hueristic, judge_state};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Player {
    One = 0,
    Two = 1,
}

impl Default for Player {
    fn default() -> Self {
        Player::One
    }
}

impl Player {
    pub fn other(&self) -> Player {
        match self {
            Player::One => Player::Two,
            Player::Two => Player::One,
        }
    }

    pub fn color(&self) -> Hueristic {
        match &self {
            Player::One => 1,
            Player::Two => -1,
        }
    }
}

pub type BitmapType = u32;
// const NUM_LEVELS: usize = 4;
pub const BOARD_WIDTH: usize = 5;
pub const NUM_SQUARES: usize = BOARD_WIDTH * BOARD_WIDTH;

pub const IS_WINNER_MASK: BitmapType = 1 << 31;
pub const MAIN_SECTION_MASK: BitmapType = (1 << 25) - 1;

pub const NEIGHBOR_MAP: [BitmapType; NUM_SQUARES] = [
    98, 229, 458, 916, 776, 3139, 7335, 14670, 29340, 24856, 100448, 234720, 469440, 938880,
    795392, 3214336, 7511040, 15022080, 30044160, 25452544, 2195456, 5472256, 10944512, 21889024,
    9175040,
];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
}

impl Serialize for Coord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

impl Coord {
    pub fn new(x: usize, y: usize) -> Self {
        Coord { x, y }
    }
}

impl Default for Coord {
    fn default() -> Self {
        Coord { x: 0, y: 0 }
    }
}

impl std::fmt::Debug for Coord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let col = match self.x {
            0 => "A",
            1 => "B",
            2 => "C",
            3 => "D",
            4 => "E",
            _ => panic!("Unknown Column: {}", self.x),
        };
        let row = 5 - self.y;
        write!(f, "{}{}", col, row)
    }
}

pub fn position_to_coord(position: usize) -> Coord {
    let x = position % 5;
    let y = position / 5;
    Coord::new(x, y)
}

#[allow(unused)]
pub fn coord_to_position(coord: Coord) -> usize {
    coord.x + coord.y * BOARD_WIDTH
}

#[allow(unused)]
fn print_full_bitmap(mut mask: BitmapType) {
    for _ in 0..5 {
        let lower = mask & 0b11111;
        let output = format!("{:05b}", lower);
        eprintln!("{}", output.chars().rev().collect::<String>());
        mask = mask >> 5;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
#[serde(rename_all(serialize = "snake_case"))]
pub enum PartialAction {
    PlaceWorker(Coord),
    SelectWorker(Coord),
    MoveWorker(Coord),
    Build(Coord),
    NoMoves,
}
type FullAction = Vec<PartialAction>;

#[derive(Clone)]
pub struct FullChoice {
    pub actions: FullAction,
    pub result_state: SantoriniState,
}

impl FullChoice {
    pub fn new(result_state: SantoriniState, action: FullAction) -> Self {
        FullChoice {
            actions: action,
            result_state,
        }
    }
}

trait ResultsMapper<T>: Clone {
    fn new() -> Self;
    fn add_action(&mut self, partial_action: PartialAction);
    fn map_result(&self, state: SantoriniState) -> T;
}

#[derive(Clone, Debug)]
struct StateOnlyMapper {}
impl ResultsMapper<SantoriniState> for StateOnlyMapper {
    fn new() -> Self {
        StateOnlyMapper {}
    }

    fn add_action(&mut self, _partial_action: PartialAction) {}

    fn map_result(&self, state: SantoriniState) -> SantoriniState {
        state
    }
}

#[derive(Clone, Debug)]
struct HueristicMapper {}
impl ResultsMapper<(SantoriniState, Hueristic)> for HueristicMapper {
    fn new() -> Self {
        HueristicMapper {}
    }

    fn add_action(&mut self, _partial_action: PartialAction) {}

    fn map_result(&self, state: SantoriniState) -> (SantoriniState, Hueristic) {
        let judge_result = judge_state(&state, 0);
        (state, judge_result)
    }
}

#[derive(Clone, Debug)]
struct FullChoiceMapper {
    partial_actions: Vec<PartialAction>,
}
impl ResultsMapper<FullChoice> for FullChoiceMapper {
    fn new() -> Self {
        FullChoiceMapper {
            partial_actions: Vec::new(),
        }
    }

    fn add_action(&mut self, partial_action: PartialAction) {
        self.partial_actions.push(partial_action);
    }

    fn map_result(&self, state: SantoriniState) -> FullChoice {
        FullChoice::new(state, self.partial_actions.clone())
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
 * bits 25-31 are unclaimed.
 */
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct SantoriniState {
    pub current_player: Player,
    // height_map[L - 1][s] represents if square s is GTE L
    pub height_map: [BitmapType; 4],
    pub workers: [BitmapType; 2],
}

impl Serialize for SantoriniState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let fen = board_to_fen(self);
        serializer.serialize_str(&fen)
    }
}

impl<'de> Deserialize<'de> for SantoriniState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fen: String = Deserialize::deserialize(deserializer)?;
        parse_fen(&fen).map_err(serde::de::Error::custom)
    }
}

pub type StateWithScore = (SantoriniState, Hueristic);

impl SantoriniState {
    pub fn new_basic_state() -> Self {
        let mut result = Self::default();
        result.workers[1] |= 1 << 7;
        result.workers[1] |= 1 << 17;
        result.workers[0] |= 1 << 12;
        result.workers[0] |= 1 << 13;
        result
    }

    pub fn flip_current_player(&mut self) {
        self.current_player = self.current_player.other()
    }

    pub fn get_height_for_worker(&self, worker_mask: BitmapType) -> usize {
        for h in (0..3).rev() {
            if self.height_map[h] & worker_mask > 0 {
                return h + 1;
            }
        }
        0
    }

    pub fn get_true_height(&self, position_mask: BitmapType) -> usize {
        for h in (0..4).rev() {
            if self.height_map[h] & position_mask > 0 {
                return h + 1;
            }
        }
        0
    }

    pub fn get_winner(&self) -> Option<Player> {
        if self.workers[0] & IS_WINNER_MASK > 0 {
            Some(Player::One)
        } else if self.workers[1] & IS_WINNER_MASK > 0 {
            Some(Player::Two)
        } else {
            None
        }
    }

    pub fn get_next_states_interactive(&self) -> Vec<FullChoice> {
        self.get_next_states_interactive_v2::<FullChoice, FullChoiceMapper>()
    }

    pub fn get_valid_next_states(&self) -> Vec<SantoriniState> {
        self.get_next_states_interactive_v2::<SantoriniState, StateOnlyMapper>()
    }

    pub fn get_next_states_with_scores(&self) -> Vec<StateWithScore> {
        self.get_next_states_interactive_v2::<StateWithScore, HueristicMapper>()
    }

    fn get_next_states_interactive_v2<T, M>(&self) -> Vec<T>
    where
        M: ResultsMapper<T>,
    {
        let mut result: Vec<T> = Vec::with_capacity(128);

        let current_player_idx = self.current_player as usize;
        let starting_current_workers = self.workers[current_player_idx] & MAIN_SECTION_MASK;
        let mut current_workers = starting_current_workers;

        let all_workers_mask = self.workers[0] | self.workers[1];

        while current_workers != 0 {
            let moving_worker_start_pos = current_workers.trailing_zeros() as usize;
            let moving_worker_start_mask: BitmapType = 1 << moving_worker_start_pos;
            current_workers ^= moving_worker_start_mask;

            let mut mapper = M::new();
            mapper.add_action(PartialAction::SelectWorker(position_to_coord(
                moving_worker_start_pos,
            )));

            let all_stable_workers = all_workers_mask ^ moving_worker_start_mask;
            let worker_starting_height = self.get_height_for_worker(moving_worker_start_mask);

            // Remember that actual height map is offset by 1
            let too_high = std::cmp::min(3, worker_starting_height + 1);
            let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos]
                & !self.height_map[too_high]
                & !all_stable_workers;

            while worker_moves != 0 {
                let worker_move_pos = worker_moves.trailing_zeros() as usize;
                let worker_move_mask: BitmapType = 1 << worker_move_pos;
                worker_moves ^= worker_move_mask;

                let mut mapper = mapper.clone();
                mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                    worker_move_pos,
                )));

                if self.height_map[2] & worker_move_mask > 0 {
                    let mut winning_next_state = self.clone();
                    winning_next_state.workers[current_player_idx] ^=
                        moving_worker_start_mask | worker_move_mask | IS_WINNER_MASK;
                    winning_next_state.flip_current_player();
                    result.push(mapper.map_result(winning_next_state));
                    continue;
                }

                let mut worker_builds =
                    NEIGHBOR_MAP[worker_move_pos] & !all_stable_workers & !self.height_map[3];

                while worker_builds != 0 {
                    let worker_build_pos = worker_builds.trailing_zeros() as usize;
                    let worker_build_mask = 1 << worker_build_pos;
                    worker_builds ^= worker_build_mask;

                    let mut mapper = mapper.clone();
                    mapper.add_action(PartialAction::Build(position_to_coord(worker_build_pos)));

                    let mut next_state = self.clone();
                    next_state.flip_current_player();
                    for height in 0.. {
                        if next_state.height_map[height] & worker_build_mask == 0 {
                            next_state.height_map[height] |= worker_build_mask;
                            break;
                        }
                    }
                    next_state.workers[current_player_idx] ^=
                        moving_worker_start_mask | worker_move_mask;
                    result.push(mapper.map_result(next_state))
                }
            }
        }

        if result.len() == 0 {
            // Lose due to no moves
            let mut next_state = self.clone();
            next_state.workers[1 - current_player_idx] |= IS_WINNER_MASK;
            next_state.flip_current_player();
            let mut mapper = M::new();
            mapper.add_action(PartialAction::NoMoves);
            result.push(mapper.map_result(next_state));
        }

        result
    }

    pub fn get_path_to_outcome(&self, other: &SantoriniState) -> Option<FullAction> {
        for choice in self.get_next_states_interactive() {
            if &choice.result_state == other {
                return Some(choice.actions);
            }
        }

        None
    }

    pub fn print_to_console(&self) {
        eprintln!("{:?}", self);

        if let Some(winner) = self.get_winner() {
            eprintln!("Player {:?} wins!", winner);
        } else {
            eprintln!("Player {:?} to play", self.current_player);
        }

        for row in 0..5 {
            let mut row_str = format!("{} ", 5 - row);
            for col in 0..5 {
                let pos = col + row * 5;
                let mask = 1 << pos;
                let height = self.get_true_height(1 << pos);

                let is_1 = self.workers[0] & mask > 0;
                let is_2 = self.workers[1] & mask > 0;

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

    pub fn get_positions_for_player(&self, player: Player) -> Vec<usize> {
        let mut result = Vec::with_capacity(2);
        let mut workers_mask = self.workers[player as usize] & MAIN_SECTION_MASK;

        while workers_mask != 0 {
            let pos = workers_mask.trailing_zeros() as usize;
            result.push(pos);
            workers_mask ^= 1 << pos;
        }

        result
    }
}

impl std::fmt::Debug for SantoriniState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", board_to_fen(self))
    }
}

impl TryFrom<&str> for SantoriniState {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}

impl TryFrom<&String> for SantoriniState {
    type Error = String;

    fn try_from(s: &String) -> Result<Self, Self::Error> {
        parse_fen(s)
    }
}
