use santorini_core::board::FullGameState;
use serde::{Deserialize, Serialize};

const CORPUS_FILE_PATH: &str = "data/corpus.toml";

#[derive(Serialize, Deserialize)]
struct StartingPosition {
    state: FullGameState,
    notes: String,
}

pub fn read_corpus() {
}
