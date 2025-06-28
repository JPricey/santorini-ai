use serde::{Deserialize, Serialize};

use crate::{
    board::FullGameState,
    gods::PartialAction,
    search::{BestMoveTrigger, Hueristic},
};
#[derive(Serialize, Deserialize, Debug)]
pub struct NextStateOutput {
    pub next_state: FullGameState,
    pub actions: Vec<PartialAction>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NextMovesOutput {
    pub start_state: FullGameState,
    pub next_states: Vec<NextStateOutput>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BestMoveMeta {
    pub score: Hueristic,
    pub calculated_depth: usize,
    pub nodes_visited: Option<usize>,
    pub elapsed_seconds: f32,
    pub actions: Vec<PartialAction>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BestMoveOutput {
    pub start_state: FullGameState,
    pub next_state: FullGameState,
    pub trigger: BestMoveTrigger,
    pub meta: BestMoveMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartedOutput {}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum EngineOutput {
    Started(StartedOutput),
    BestMove(BestMoveOutput),
    NextMoves(NextMovesOutput),
}
