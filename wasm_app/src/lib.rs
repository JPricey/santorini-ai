use js_sys;
use santorini_core::{
    board::FullGameState,
    fen::parse_fen,
    gods::PartialAction,
    matchup::BANNED_MATCHUPS,
    player::Player,
    pretty_board::{game_state_with_partial_actions, state_to_pretty_board},
    search::{SearchContext, get_past_win_search_terminator, negamax_search},
    search_terminators::SearchTerminator,
    transposition_table::TranspositionTable,
    uci_types::{BestMoveMeta, BestMoveOutput, EngineOutput, NextMovesOutput, NextStateOutput},
    utils::find_action_path,
};
use serde::{Deserialize, Serialize};
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

fn _parse_js_number(number: &JsValue) -> Result<f64, String> {
    let Some(number) = number.as_f64() else {
        return Err("Could not parse number".to_owned());
    };

    Ok(number)
}

fn _parse_fen_js_value(fen: &JsValue) -> Result<FullGameState, String> {
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

#[wasm_bindgen]
#[allow(non_snake_case)]
impl WasmApp {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            tt: TranspositionTable::new(),
        }
    }

    fn _computeNextMoveResult(
        &mut self,
        fen: JsValue,
        duration: JsValue,
    ) -> Result<JsValue, String> {
        let timeLimit = _parse_js_number(&duration)?;
        let state = _parse_fen_js_value(&fen)?;

        let mut search_state = SearchContext {
            tt: &mut self.tt,
            new_best_move_callback: Box::new(|_| {}),
            terminator: JsTimeSearchTerminator::new(timeLimit),
        };

        let search_result = negamax_search(
            &mut search_state,
            state.clone(),
            get_past_win_search_terminator(),
        );

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

            return serde_wasm_bindgen::to_value(&output).map_err(|e| e.to_string());
        } else {
            return Err(format!(
                "no move {} {}",
                search_result.last_fully_completed_depth, search_result.nodes_visited
            ));
        }
    }

    pub fn computeNextMove(&mut self, fen: JsValue, duration: JsValue) -> JsValue {
        match self._computeNextMoveResult(fen, duration) {
            Ok(result) => result,
            Err(err) => JsValue::from(err),
        }
    }
}

pub fn _get_next_moves_interactive_result(fen: JsValue) -> Result<JsValue, String> {
    let state = _parse_fen_js_value(&fen)?;
    let fen_string = JsValue::as_string(&fen).ok_or("fen must be a string")?;

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

    serde_wasm_bindgen::to_value(&output).map_err(|e| e.to_string())
}

#[wasm_bindgen]
pub fn get_next_moves_interactive(fen: JsValue) -> JsValue {
    match _get_next_moves_interactive_result(fen) {
        Ok(result) => result,
        Err(err) => JsValue::from(err),
    }
}

#[wasm_bindgen]
pub fn get_banned_matchups() -> JsValue {
    let mut res: Vec<String> = Vec::new();
    for matchup in BANNED_MATCHUPS.keys() {
        let matchup_str = format!("{}|{}", matchup.gods[0], matchup.gods[1]);
        res.push(matchup_str);
    }

    serde_wasm_bindgen::to_value(&res).unwrap_or_else(|e| JsValue::from_str(&format!("{:?}", e)))
}

fn _get_player_strings_inner(fen: JsValue) -> Result<JsValue, String> {
    let state = _parse_fen_js_value(&fen)?;

    let p1_string = state.gods[0].pretty_stringify_god_data(&state.board, Player::One);
    let p2_string = state.gods[1].pretty_stringify_god_data(&state.board, Player::Two);

    let res = (p1_string, p2_string);
    serde_wasm_bindgen::to_value(&res).map_err(|e| format!("{:?}", e))
}

#[wasm_bindgen]
pub fn get_player_strings(fen: JsValue) -> JsValue {
    _get_player_strings_inner(fen).unwrap_or_else(|e| JsValue::from_str(&e))
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
struct PrettyGameStateArgs {
    fen: String,
    actions: Option<Vec<PartialAction>>,
}

fn _get_pretty_game_state_inner(args: JsValue) -> Result<JsValue, String> {
    let args =
        serde_wasm_bindgen::from_value::<PrettyGameStateArgs>(args).map_err(|e| e.to_string())?;

    let mut state = parse_fen(&args.fen)?;

    if let Some(actions) = args.actions {
        state = game_state_with_partial_actions(&state, &actions);
    };

    let pretty_board = state_to_pretty_board(&state);

    serde_wasm_bindgen::to_value(&pretty_board).map_err(|e| e.to_string())
}

#[wasm_bindgen]
pub fn get_pretty_game_state(args: JsValue) -> JsValue {
    _get_pretty_game_state_inner(args).unwrap_or_else(|e| JsValue::from_str(&e))
}
