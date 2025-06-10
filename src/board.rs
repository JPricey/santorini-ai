use colored::{Colorize};

use super::search::{Hueristic, judge_state};

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
const BOARD_WIDTH: usize = 5;
const NUM_SQUARES: usize = BOARD_WIDTH * BOARD_WIDTH;

pub const IS_WINNER_MASK: BitmapType = 1 << 31;
pub const MAIN_SECTION_MASK: BitmapType = (1 << 25) - 1;

pub const NEIGHBOR_MAP: [BitmapType; NUM_SQUARES] = [
    98, 229, 458, 916, 776, 3139, 7335, 14670, 29340, 24856, 100448, 234720, 469440, 938880,
    795392, 3214336, 7511040, 15022080, 30044160, 25452544, 2195456, 5472256, 10944512, 21889024,
    9175040,
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Coord {
    pub x: usize,
    pub y: usize,
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

fn position_to_coord(position: usize) -> Coord {
    let x = position % 5;
    let y = position / 5;
    Coord::new(x, y)
}

#[allow(unused)]
fn coord_to_position(coord: Coord) -> usize {
    coord.x + coord.y * BOARD_WIDTH
}

#[allow(unused)]
fn print_full_bitmap(mut mask: BitmapType) {
    for _ in 0..5 {
        let lower = mask & 0b11111;
        let output = format!("{:05b}", lower);
        println!("{}", output.chars().rev().collect::<String>());
        mask = mask >> 5;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
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
    pub action: FullAction,
    pub result_state: SantoriniState,
}

impl FullChoice {
    pub fn new(result_state: SantoriniState, action: FullAction) -> Self {
        FullChoice {
            action,
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

impl SantoriniState {
    pub fn new_basic_state() -> Self {
        let mut result = Self::default();
        result.workers[0] |= 1 << 7;
        result.workers[0] |= 1 << 17;
        result.workers[1] |= 1 << 11;
        result.workers[1] |= 1 << 13;
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

    pub fn get_next_states_with_scores(&self) -> Vec<(SantoriniState, Hueristic)> {
        self.get_next_states_interactive_v2::<(SantoriniState, Hueristic), HueristicMapper>()
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
            return vec![mapper.map_result(next_state)];
        }

        result
    }

    pub fn get_path_to_outcome(&self, other: &SantoriniState) -> Option<FullAction> {
        for choice in self.get_next_states_interactive() {
            if &choice.result_state == other {
                return Some(choice.action);
            }
        }

        None
    }

    pub fn print_to_console(&self) {
        if let Some(winner) = self.get_winner() {
            println!("Player {:?} wins!", winner);
        } else {
            println!("Player {:?} to play", self.current_player);
        }

        for row in 0..5 {
            print!("{}", 5 - row);

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

                let with_color = match height {
                    0 => char.on_white(),
                    1 => char.on_yellow(),
                    2 => char.on_blue(),
                    3 => char.on_green(),
                    4 => char.on_black(),
                    _ => panic!("Invalid Height: {}", height),
                };
                print!("{}", with_color);
            }
            println!()
        }
        println!(" ABCDE");
    }
}

impl TryFrom<&str> for SantoriniState {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let sections: Vec<&str> = s.split('/').collect();
        if sections.len() != 3 {
            return Err("Input string must have exactly 3 sections separated by '/'".to_string());
        }

        let mut result = SantoriniState::default();

        let heights = sections[0];

        if heights.len() != NUM_SQUARES {
            return Err("Height map must be exactly 25 characters".to_string());
        }

        for (p, char) in heights.chars().enumerate() {
            if char < '0' || char > '4' {
                return Err(format!("Invalid character '{}' at position {}", char, p));
            }
            let height = (char as u8 - b'0') as usize;
            for h in 0..height {
                result.height_map[h] |= 1 << p;
            }
        }

        let p1_positions = sections[1];
        for p1_pos in p1_positions.split(',') {
            if p1_pos.is_empty() {
                continue;
            }
            let pos: usize = p1_pos
                .parse()
                .map_err(|_| format!("Invalid position '{}'", p1_pos))?;
            if pos >= NUM_SQUARES {
                return Err(format!("Position {} out of bounds", pos));
            }
            result.workers[0] |= 1 << pos;
        }

        let p2_positions = sections[2];
        for p2_pos in p2_positions.split(',') {
            if p2_pos.is_empty() {
                continue;
            }
            let pos: usize = p2_pos
                .parse()
                .map_err(|_| format!("Invalid position '{}'", p2_pos))?;
            if pos >= NUM_SQUARES {
                return Err(format!("Position {} out of bounds", pos));
            }
            result.workers[1] |= 1 << pos;
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::*;

    #[test]
    fn benchmark_finding_children_with_hueristic() {
        let state = SantoriniState::new_basic_state();
        let start_time = std::time::Instant::now();
        for _ in 0..1000000 {
            black_box(state.get_next_states_with_scores());
        }
        let elapsed = start_time.elapsed();
        println!("v2: {} ms", elapsed.as_millis());
    }

    #[test]
    fn benchmark_finding_children_fast() {
        let state = SantoriniState::new_basic_state();
        let start_time = std::time::Instant::now();
        for _ in 0..1000000 {
            black_box(state.get_valid_next_states());
        }
        let elapsed = start_time.elapsed();
        println!("fast: {} ms", elapsed.as_millis());
    }

    #[test]
    fn benchmark_finding_children_interactive() {
        let state = SantoriniState::new_basic_state();
        let start_time = std::time::Instant::now();
        for _ in 0..1000000 {
            black_box(state.get_next_states_interactive());
        }
        let elapsed = start_time.elapsed();
        println!("interactive: {} ms", elapsed.as_millis());
    }

    #[test]
    fn output_neighbor_mask() {
        // Script to output neighbor mask
        // if true { return }
        for p in 0..NUM_SQUARES {
            let coord = position_to_coord(p);
            let (x, y) = (coord.x as i64, coord.y as i64);

            let mut neighbor_mask = 0 as BitmapType;
            for dx in [-1, 0, 1] {
                for dy in [-1, 0, 1] {
                    if dx == dy && dx == 0 {
                        continue;
                    }

                    let nx = x + dx;
                    let ny = y + dy;

                    if nx < 0 || nx >= BOARD_WIDTH as i64 || ny < 0 || ny >= BOARD_WIDTH as i64 {
                        continue;
                    }

                    let nc: usize = coord_to_position(Coord::new(nx as usize, ny as usize));
                    neighbor_mask |= 1 << nc;
                }
            }
            println!("{},", neighbor_mask);

            // println!("{:?}", coord);
            // print_full_bitmap(neighbor_mask);
        }
    }
}
