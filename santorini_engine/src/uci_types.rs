use serde::{Deserialize, Serialize};

use crate::{
    board::{PartialAction, SantoriniState},
    search::Hueristic,
};
#[derive(Serialize, Deserialize, Debug)]
pub struct NextStateOutput {
    pub next_state: SantoriniState,
    pub actions: Vec<PartialAction>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NextMovesOutput {
    pub start_state: SantoriniState,
    pub next_states: Vec<NextStateOutput>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BestMoveMeta {
    pub score: Hueristic,
    pub calculated_depth: usize,
    pub elapsed_seconds: f32,
    pub actions: Vec<PartialAction>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BestMoveOutput {
    pub start_state: SantoriniState,
    pub next_state: SantoriniState,
    pub meta: BestMoveMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartedOutput {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all(serialize = "snake_case"))]
pub enum EngineOutput {
    Started(StartedOutput),
    BestMove(BestMoveOutput),
    NextMoves(NextMovesOutput),
}
