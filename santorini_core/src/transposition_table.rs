use std::{
    hash::{DefaultHasher, Hash, Hasher},
    u8,
};

use crate::{
    board::FullGameState,
    gods::generic::GenericMove,
    hashing::{HashType, compute_hash_from_scratch},
    search::{MAX_PLY, WINNING_SCORE_BUFFER},
};

use super::search::Heuristic;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchScoreType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone, Debug)]
pub struct TTValue {
    // TODO: should be best action?
    pub best_action: GenericMove,
    pub search_depth: u8,
    pub score_type: SearchScoreType,
    pub score: Heuristic,
    pub eval: Heuristic,
}

impl Default for TTValue {
    fn default() -> Self {
        TTValue {
            best_action: GenericMove::NULL_MOVE,
            search_depth: 0,
            score_type: SearchScoreType::LowerBound,
            score: 0,
            eval: 0,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct TTEntry {
    pub hash_code: HashType,
    pub value: TTValue,
    // pub board: BoardState,
}

fn to_tt(value: Heuristic, ply: usize) -> Heuristic {
    let ply = ply as Heuristic;

    if value >= WINNING_SCORE_BUFFER {
        value + ply
    } else if value <= -WINNING_SCORE_BUFFER {
        value - ply
    } else {
        value
    }
}

fn to_search(value: Heuristic, ply: usize) -> Heuristic {
    let ply = ply as Heuristic;

    if value >= WINNING_SCORE_BUFFER {
        value - ply
    } else if value <= -WINNING_SCORE_BUFFER {
        value + ply
    } else {
        value
    }
}

pub struct TranspositionTable {
    pub lmr_table: LMRTable,
    pub entries: Vec<TTEntry>,
    pub stats: TTStats,
}

// const TABLE_SIZE: HashType = 999_983;
const TABLE_SIZE: HashType = 5_000_011;
// const TABLE_SIZE: HashType = 10_000_019;
// const TABLE_SIZE: HashType = 22_633_363; // 1 GB
// const TABLE_SIZE: HashType = 100_000_007; // too big

fn _hash_obj<T>(obj: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    obj.hash(&mut hasher);
    hasher.finish()
}

fn hash_board(state: &FullGameState) -> HashType {
    debug_assert_eq!(state.board.hash, compute_hash_from_scratch(state));
    state.board.hash
    // _hash_obj(board)
}

#[derive(Clone, Debug, Default)]
pub struct TTStats {
    pub insert: usize,
    pub hit: usize,
    pub missed: usize,
    pub read_collision: usize,
    pub used_value: usize,
    pub unused_value: usize,
}

impl TranspositionTable {
    pub const IS_TRACKING_STATS: bool = false;

    pub fn new() -> Self {
        Self {
            lmr_table: LMRTable::new(),
            entries: vec![
                TTEntry {
                    hash_code: 0,
                    value: TTValue::default(),
                };
                TABLE_SIZE as usize
            ],
            stats: Default::default(),
        }
    }

    /// Get a key that wraps around the table size, avoiding using Modulo.
    /// https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
    fn get_key(&self, hash: u64) -> usize {
        (hash % TABLE_SIZE) as usize
        // let key = hash as u128;
        // let len = TABLE_SIZE as u128;

        // ((key * len) >> 64) as usize
    }

    pub fn insert(
        &mut self,
        state: &FullGameState,
        best_action: GenericMove,
        depth: u8,
        score_type: SearchScoreType,
        search_score: Heuristic,
        current_eval: Heuristic,
        ply: usize,
    ) {
        let hash_code = hash_board(state);
        let destination = self.get_key(hash_code);

        let new_entry = TTEntry {
            value: TTValue {
                best_action,
                search_depth: depth,
                score_type,
                score: to_tt(search_score, ply),
                eval: current_eval,
            },
            hash_code,
        };

        self.entries[destination] = new_entry;

        if TranspositionTable::IS_TRACKING_STATS {
            self.stats.insert += 1;
        }
    }

    pub fn conditionally_insert(
        &mut self,
        state: &FullGameState,
        mut best_action: GenericMove,
        depth: u8,
        score_type: SearchScoreType,
        search_score: Heuristic,
        current_eval: Heuristic,
        ply: usize,
    ) {
        let hash_code = hash_board(state);
        let destination = self.get_key(hash_code);

        let old_entry = &mut self.entries[destination];
        if old_entry.hash_code == hash_code {
            if old_entry.value.search_depth >= depth {
                return;
            }

            best_action = old_entry.value.best_action;
        }

        let new_entry = TTEntry {
            value: TTValue {
                best_action,
                search_depth: depth,
                score_type,
                score: to_tt(search_score, ply),
                eval: current_eval,
            },
            hash_code,
        };

        self.entries[destination] = new_entry;

        if TranspositionTable::IS_TRACKING_STATS {
            self.stats.insert += 1;
        }
    }

    pub fn fetch(&mut self, state: &FullGameState, ply: usize) -> Option<TTValue> {
        let hash_code = hash_board(state);
        let destination = self.get_key(hash_code);

        let entry = &self.entries[destination];
        if entry.hash_code == hash_code {
            if TranspositionTable::IS_TRACKING_STATS {
                self.stats.hit += 1;
            }

            return Some(TTValue {
                best_action: entry.value.best_action,
                search_depth: entry.value.search_depth,
                score_type: entry.value.score_type,
                score: to_search(entry.value.score, ply),
                eval: entry.value.eval,
            });
        } else if TranspositionTable::IS_TRACKING_STATS {
            // eprintln!("TT COLLISION: {}", hash_code);
            // state.print_to_console();
            // entry.board.print_to_console();
            if entry.hash_code == 0 {
                self.stats.missed += 1;
            } else {
                self.stats.read_collision += 1;
            }
        }
        None
    }

    pub fn count_filled_entries(&self) -> usize {
        self.entries.iter().filter(|e| e.hash_code != 0).count()
    }

    pub fn reset(&mut self) {
        self.entries
            .iter_mut()
            .for_each(|entry| *entry = Default::default());
        self.stats = Default::default();
    }
}

impl std::fmt::Debug for TranspositionTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fill_pct = (self.count_filled_entries() * 100 / self.entries.len()) as f32 / 100.0;

        f.debug_struct("TTable")
            .field("stats", &self.stats)
            .field("fill_pct", &fill_pct)
            .finish()
    }
}

const LMR_MAX_MOVES: usize = 1536;
pub struct LMRTable {
    pub table: Vec<[i32; LMR_MAX_MOVES + 1]>,
}

impl LMRTable {
    pub fn new() -> Self {
        let mut res = Self::default();

        for depth in 1..MAX_PLY + 1 {
            for played in 1..LMR_MAX_MOVES + 1 {
                let ld = f64::ln(depth as f64);
                let lp = f64::ln(played as f64);
                res.table[depth][played] = (1024.0 * (-2.2 + ld * lp / 2.2).max(-2.0)) as i32;
            }
        }

        res
    }

    pub fn get(&self, depth: usize, played: usize) -> i32 {
        self.table[depth.min(MAX_PLY)][played.min(LMR_MAX_MOVES)]
    }
}

impl Default for LMRTable {
    fn default() -> Self {
        Self {
            table: vec![[0; LMR_MAX_MOVES + 1]; MAX_PLY + 1],
        }
    }
}
