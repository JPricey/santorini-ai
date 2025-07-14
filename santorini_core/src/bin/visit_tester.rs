use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use santorini_core::{
    board::FullGameState,
    gods::generic::GenericMove,
    search::{Hueristic, SearchContext, SearchState, WINNING_SCORE_BUFFER, negamax_search},
    search_terminators::DynamicMaxDepthSearchTerminator,
    transposition_table::TranspositionTable,
    utils::SEARCH_TEST_SCENARIOS,
};

// const MAX_SECS_PER_TEST: Duration = Cur
const TUNE_UNTIL_ABOVE_SECS: Duration = Duration::from_secs(2);

#[derive(Serialize, Deserialize, Debug)]
struct ScenarioEntry {
    duration_seconds: f32,
    nodes_visited: usize,
    best_move: GenericMove,
    best_move_str: String,
    best_move_state: FullGameState,
    score: Hueristic,
    depth: usize,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct TestSummary {
    all_scenarios: Vec<ScenarioEntry>,
    duration_sum: f32,
    nodes_visited_sum: usize,
}

fn run_scenario(
    tt: &mut TranspositionTable,
    game_state: &FullGameState,
    depth: usize,
) -> (SearchState, Duration) {
    let mut search_state = SearchContext::new(tt, DynamicMaxDepthSearchTerminator::new(depth));

    let start_at = Instant::now();
    let result = negamax_search(&mut search_state, &game_state);
    let duration = start_at.elapsed();

    (result, duration)
}

#[allow(dead_code)]
fn tune_scenario_depths() {
    let mut scenarios = SEARCH_TEST_SCENARIOS.clone();
    let mut tt = TranspositionTable::new();

    for i in 0..scenarios.len() {
        let (state_str, mut depth) = SEARCH_TEST_SCENARIOS[i];
        let game_state = FullGameState::try_from(state_str).unwrap();

        loop {
            tt.reset();
            let (result, duration) = run_scenario(&mut tt, &game_state, depth);

            eprintln!("{i}: {:.2} - {:?}", duration.as_secs_f32(), result);
            if duration < TUNE_UNTIL_ABOVE_SECS {
                if result.best_move.unwrap().score.abs() > WINNING_SCORE_BUFFER {
                    break;
                }
                depth += 1;
                scenarios[i].1 = depth;
                eprint!("too short, trying again: {}", depth);
            } else {
                break;
            }
        }
    }

    eprintln!("");
    eprintln!("{:?}", scenarios);
}

fn run_all_scenarios() -> TestSummary {
    let mut tt = TranspositionTable::new();
    let mut summary = TestSummary::default();

    for (i, (state_str, depth)) in SEARCH_TEST_SCENARIOS.iter().cloned().enumerate() {
        let game_state = FullGameState::try_from(state_str).unwrap();

        let depth = (depth as i32 - 4).max(5) as usize;

        tt.reset();
        let (result, duration) = run_scenario(&mut tt, &game_state, depth);

        let best_move = result.best_move.clone().unwrap();

        let scenario_entry = ScenarioEntry {
            duration_seconds: duration.as_secs_f32(),
            nodes_visited: result.nodes_visited,
            best_move: best_move.action,
            best_move_str: format!("{:?}", best_move.action),
            best_move_state: best_move.child_state,
            score: best_move.score,
            depth: best_move.depth,
        };

        eprintln!("{i}: {:?}", scenario_entry);

        summary.all_scenarios.push(scenario_entry);
        summary.duration_sum += duration.as_secs_f32();
        summary.nodes_visited_sum += result.nodes_visited;
    }

    summary
}

pub fn main() {
    let summary = run_all_scenarios();
    // eprint!("{:?}", summary);

    let toml_string = serde_yaml::to_string(&summary).expect("Failed to serialize summary");
    std::fs::write("data/move_test.yaml", toml_string).expect("Failed to write corpus to file");
}
