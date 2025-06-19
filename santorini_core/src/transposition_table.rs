use std::hash::{DefaultHasher, Hash, Hasher};

use super::{board::BoardState, search::Hueristic};

pub type HashCodeType = u64;

#[derive(Clone, Debug)]
pub enum SearchScoreType {
    Exact,
    LowerBound,
    UpperBound,
}

#[derive(Clone)]
pub struct TTValue {
    // TODO: should be best action?
    pub best_child: BoardState,
    pub search_depth: u8,
    pub score_type: SearchScoreType,
    pub score: Hueristic,
}

#[derive(Clone)]
pub struct TTEntry {
    pub hash_code: HashCodeType,
    pub value: TTValue,
}

pub struct TranspositionTable {
    pub entries: Vec<Option<TTEntry>>,
    pub stats: TTStats,
}

// const TABLE_SIZE: HashCodeType = 999983;
const TABLE_SIZE: HashCodeType = 22_633_363; // 1 GB
// const TABLE_SIZE: HashCodeType = 100_000_007; // too big

fn hash_obj<T>(obj: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    obj.hash(&mut hasher);
    hasher.finish()
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
            entries: vec![None; TABLE_SIZE as usize],
            stats: Default::default(),
        }
    }

    pub fn insert(&mut self, state: &BoardState, value: TTValue) {
        if TranspositionTable::IS_TRACKING_STATS {
            self.stats.insert += 1;
        }

        let hash_code = hash_obj(state);
        let destination = hash_code % TABLE_SIZE;

        self.entries[destination as usize] = Some(TTEntry { hash_code, value });
    }

    pub fn fetch(&mut self, state: &BoardState) -> Option<&TTValue> {
        let hash_code = hash_obj(state);
        let destination = hash_code % TABLE_SIZE;

        if let Some(entry) = &self.entries[destination as usize] {
            if entry.hash_code == hash_code {
                if TranspositionTable::IS_TRACKING_STATS {
                    self.stats.hit += 1;
                }

                return Some(&entry.value);
            } else if TranspositionTable::IS_TRACKING_STATS {
                self.stats.read_collision += 1;
            }
        }
        if TranspositionTable::IS_TRACKING_STATS {
            self.stats.missed += 1;
        }
        None
    }

    pub fn count_filled_entries(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    pub fn reset(&mut self) {
        self.entries.fill(None);
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
