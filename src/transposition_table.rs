use std::hash::{DefaultHasher, Hash, Hasher};

use super::{board::SantoriniState, search::Hueristic};

pub type HashCodeType = u64;

#[derive(Clone, Debug)]
pub enum SearchScore {
    Exact(Hueristic),
    LowerBound(Hueristic),
    UpperBound(Hueristic),
}

#[derive(Clone)]
pub struct TTValue {
    // TODO: should be best action? 
    pub best_child: SantoriniState,
    pub search_depth: u8,
    pub score: SearchScore,
}

#[derive(Clone)]
pub struct Entry {
    pub hash_code: HashCodeType,
    pub value: TTValue,
}

pub struct TranspositionTable {
    pub entries: Vec<Option<Entry>>,
    pub stats: TTStats,
}

// const TABLE_SIZE: HashCodeType = 999983;
const TABLE_SIZE: HashCodeType = 22633363;

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
    pub const IS_TRACKING_STATS: bool = true;

    pub fn new() -> Self {
        Self {
            entries: vec![None; TABLE_SIZE as usize],
            stats: Default::default(),
        }
    }

    pub fn insert(&mut self, state: &SantoriniState, value: TTValue) {
        if TranspositionTable::IS_TRACKING_STATS {
            self.stats.insert += 1;
        }

        let hash_code = hash_obj(state);
        let destination = hash_code % TABLE_SIZE;

        self.entries[destination as usize] = Some(Entry { hash_code, value });
    }

    pub fn fetch(&mut self, state: &SantoriniState) -> Option<&TTValue> {
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
}
