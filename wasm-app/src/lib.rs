use js_sys;
use santorini_core::{
    board::FullGameState,
    search::{SearchContext, negamax_search},
    search_terminators::{
        DynamicNodesVisitedSearchTerminator, SearchTerminator, StaticNodesVisitedSearchTerminator,
    },
    transposition_table::TranspositionTable,
    uci_types::{BestMoveMeta, BestMoveOutput, EngineOutput, NextMovesOutput, NextStateOutput},
    utils::find_action_path,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

fn current_ms() -> f64 {
    let now = js_sys::Date::now();
    now
}

const CHECK_PER_NODES: usize = 10_000;

struct JsTimeSearchTerminator {
    is_done: bool,
    time_limit_ms: f64,
    started_at: f64,
    next_node_count_check: usize,
}
impl SearchTerminator for JsTimeSearchTerminator {
    fn should_stop(&mut self, search_state: &santorini_core::search::SearchState) -> bool {
        if !self.is_done && search_state.nodes_visited >= self.next_node_count_check {
            self.next_node_count_check = search_state.nodes_visited + CHECK_PER_NODES;
            let now = current_ms();
            self.is_done = now >= self.started_at + self.time_limit_ms;
        }

        self.is_done
    }
}
impl JsTimeSearchTerminator {
    pub fn new(time_limit_ms: f64) -> Self {
        Self {
            is_done: false,
            time_limit_ms,
            started_at: current_ms(),
            next_node_count_check: CHECK_PER_NODES,
        }
    }
}

/// Wraps the Rust application state and exposes it to JavaScript
#[wasm_bindgen]
pub struct WasmApp {
    tt: TranspositionTable,
}

fn _parseJsNumber(number: &JsValue) -> Result<f64, String> {
    let Some(number) = number.as_f64() else {
        return Err("Could not parse number".to_owned());
    };

    Ok(number)
}

fn _parseFenJsValue(fen: &JsValue) -> Result<FullGameState, String> {
    let Some(fen) = fen.as_string() else {
        return Err("fen must be a string".to_owned());
    };
    let state = match FullGameState::try_from(&fen) {
        Ok(state) => state,
        Err(err) => return Err(format!("Error parsing fen: {}", err)),
    };
    if state.board.get_winner().is_some() {
        return Err("board is already terminal".to_owned());
    }

    return Ok(state);
}

// struct SearchResult {
//     next_state: String,
// }

#[wasm_bindgen]
#[allow(non_snake_case)]
impl WasmApp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            tt: TranspositionTable::new(),
        }
    }

    pub fn computeNextMove(&mut self, fen: JsValue, duration: JsValue) -> JsValue {
        let timeLimit = match _parseJsNumber(&duration) {
            Ok(state) => state,
            Err(err) => return JsValue::from(err),
        };
        let state = match _parseFenJsValue(&fen) {
            Ok(state) => state,
            Err(err) => return JsValue::from(err),
        };

        let mut search_state = SearchContext {
            tt: &mut self.tt,
            new_best_move_callback: Box::new(|_| {}),
            terminator: JsTimeSearchTerminator::new(timeLimit),
        };

        let search_result = negamax_search(&mut search_state, &state);

        if let Some(action) = search_result.best_move {
            let actions = find_action_path(&state, &action.child_state).unwrap_or_default();

            let meta = BestMoveMeta {
                score: action.score,
                calculated_depth: action.depth,
                nodes_visited: Some(action.nodes_visited),
                elapsed_seconds: 0.0,
                actions: actions,
                action_str: Some(action.action_str),
            };

            let output = BestMoveOutput {
                original_str: Some(JsValue::as_string(&fen).unwrap()),
                start_state: state.clone(),
                next_state: action.child_state,
                trigger: action.trigger,
                meta: meta,
            };

            return JsValue::from_serde(&output).unwrap();
        } else {
            return JsValue::from(format!(
                "no move {} {}",
                search_result.last_fully_completed_depth, search_result.nodes_visited
            ));
        }
    }
}
#[wasm_bindgen]
pub fn getNextMovesInteractive(fen: JsValue) -> JsValue {
    let state = match _parseFenJsValue(&fen) {
        Ok(state) => state,
        Err(err) => return JsValue::from(err),
    };

    let fen_string = JsValue::as_string(&fen).unwrap();

    let child_states = state.get_next_states_interactive();
    let output = EngineOutput::NextMoves(NextMovesOutput {
        original_str: Some(fen_string),
        start_state: state,
        next_states: child_states
            .into_iter()
            .map(|full_choice| NextStateOutput {
                next_state: full_choice.state,
                actions: full_choice.actions,
            })
            .collect(),
    });

    if let Ok(output) = JsValue::from_serde(&output) {
        output
    } else {
        return JsValue::from("Could not serialize output");
    }
}
