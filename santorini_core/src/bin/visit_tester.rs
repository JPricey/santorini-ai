use clap::Parser;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

use santorini_core::{
    board::FullGameState,
    gods::{GodName, generic::GenericMove},
    search::{Hueristic, SearchContext, SearchState, WINNING_SCORE_BUFFER, negamax_search},
    search_terminators::DynamicMaxDepthSearchTerminator,
    transposition_table::TranspositionTable,
    utils::SEARCH_TEST_SCENARIOS,
};

const TUNE_UNTIL_ABOVE_SECS: Duration = Duration::from_secs(2);

#[derive(Serialize, Deserialize, Debug)]
struct ScenarioEntry {
    index: usize,
    state: FullGameState,
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
    let result = negamax_search(&mut search_state, game_state.clone());
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

fn run_all_scenarios(args: VisitTesterArgs) -> TestSummary {
    let mut tt = TranspositionTable::new();
    let mut summary = TestSummary::default();

    let VisitTesterArgs {
        god,
        reduction,
        minimum,
        scenario,
        save: _,
    } = args;

    for (i, (state_str, depth)) in SEARCH_TEST_SCENARIOS.iter().cloned().enumerate() {
        if scenario >= 0 && i as i32 != scenario {
            continue;
        }

        let mut game_state = FullGameState::try_from(state_str).unwrap();
        game_state.gods[0] = god.to_power();
        game_state.gods[1] = god.to_power();
        game_state.recalculate_internals();

        let depth = (depth as i32 - reduction as i32).max(minimum as i32) as usize;

        eprintln!("{i}: {depth} {:?}", game_state);

        tt.reset();
        let (result, duration) = run_scenario(&mut tt, &game_state, depth);

        let best_move = result.best_move.clone().unwrap();

        let scenario_entry = ScenarioEntry {
            index: i,
            state: game_state,
            duration_seconds: duration.as_secs_f32(),
            nodes_visited: result.nodes_visited,
            best_move: best_move.action,
            best_move_str: best_move.action_str.clone(),
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

#[derive(Parser, Debug, Clone, Copy)]
struct VisitTesterArgs {
    #[arg(short = 'g', long)]
    #[clap(default_value_t = GodName::Mortal)]
    god: GodName,

    #[arg(short = 'r', long)]
    #[clap(default_value_t = 0)]
    reduction: usize,

    #[arg(short = 'm', long)]
    #[clap(default_value_t = 4)]
    minimum: usize,

    #[arg(short = 's', long)]
    #[clap(default_value_t = false)]
    save: bool,

    #[arg(short = 'c', long)]
    #[clap(default_value_t = -1)]
    scenario: i32,
}

pub fn main() {
    let args = VisitTesterArgs::parse();
    let summary = run_all_scenarios(args);

    eprintln!(
        "Nodes Visited: {} Duration sum: {:.2}",
        summary.nodes_visited_sum, summary.duration_sum
    );

    if args.save {
        let toml_string = serde_yaml::to_string(&summary).expect("Failed to serialize summary");
        std::fs::write("data/move_test.yaml", toml_string).expect("Failed to write corpus to file");
    }
}

// cargo run -p santorini_core --bin visit_tester --release -- -g mortal -r 4 -m 4
// Nodes Visited: 24659694 Duration sum: 12.22
// cargo run -p santorini_core --bin visit_tester --release -- -g pan -r 0 -m 4
// Nodes Visited: 16075005 Duration sum: 9.13
// cargo run -p santorini_core --bin visit_tester --release -- -g artemis -r 7 -m 4
// Nodes Visited: 15354454 Duration sum: 10.55
// cargo run -p santorini_core --bin visit_tester --release -- -g hephaestus -r 0 -m 4
// Nodes Visited: 10007272 Duration sum: 5.79
// cargo run -p santorini_core --bin visit_tester --release -- -g atlas -r 6 -m 4
// Nodes Visited: 28070176 Duration sum: 15.19
// cargo run -p santorini_core --bin visit_tester --release -- -g athena -r 7 -m 4
// Nodes Visited: 30654927 Duration sum: 16.35
// cargo run -p santorini_core --bin visit_tester --release -- -g minotaur -r 17 -m 6
// Nodes Visited: 33504776 Duration sum: 13.79
// cargo run -p santorini_core --bin visit_tester --release -- -g demeter -r 7 -m 4
// Nodes Visited: 13947635 Duration sum: 7.37
// cargo run -p santorini_core --bin visit_tester --release -- -g apollo -r 20 -m 3
// Nodes Visited: 11168757 Duration sum: 6.24
// cargo run -p santorini_core --bin visit_tester --release -- -g hermes -r 20 -m 4
// Nodes Visited: 3282845 Duration sum: 11.12
// cargo run -p santorini_core --bin visit_tester --release -- -g prometheus -r 10 -m 4
// Nodes Visited: 5186548 Duration sum: 5.71
// cargo run -p santorini_core --bin visit_tester --release -- -g urania -r 8 -m 4
// Nodes Visited: 1180074 Duration sum: 1.08
// cargo run -p santorini_core --bin visit_tester --release -- -g graeae -r 6 -m 4
// Nodes Visited: 1778873 Duration sum: 1.39
