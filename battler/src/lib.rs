use santorini_core::board::FullGameState;
use serde::{Deserialize, Serialize};

const CORPUS_FILE_PATH: &str = "data/corpus.yaml";

#[derive(Serialize, Deserialize)]
pub struct StartingPosition {
    pub name: String,
    pub state: FullGameState,
    pub notes: String,
}

#[derive(Serialize, Deserialize)]
pub struct Corpus {
    pub positions: Vec<StartingPosition>,
}

pub fn write_corpus(corpus: &Corpus) {
    let toml_string = serde_yaml::to_string(corpus).expect("Failed to serialize corpus");
    std::fs::write(CORPUS_FILE_PATH, toml_string).expect("Failed to write corpus to file");
}

pub fn read_corpus() -> Corpus {
    let toml_string =
        std::fs::read_to_string(CORPUS_FILE_PATH).expect("Failed to read corpus file");
    serde_yaml::from_str(&toml_string).expect("Failed to deserialize corpus")
}
