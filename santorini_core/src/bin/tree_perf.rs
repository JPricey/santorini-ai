use clap::Parser;
use std::time::Instant;

use santorini_core::{
    board::FullGameState,
    search::{SearchContext, SearchState, negamax_search},
    search_terminators::DynamicMaxDepthSearchTerminator,
    transposition_table::TranspositionTable,
    utils::SEARCH_TEST_SCENARIOS,
};

fn test(tt: &mut TranspositionTable, scenario: usize) -> SearchState {
    let (state_str, depth) = SEARCH_TEST_SCENARIOS[scenario];
    let game_state = FullGameState::try_from(state_str).unwrap();
    let mut search_state = SearchContext::new(tt, DynamicMaxDepthSearchTerminator::new(depth));

    negamax_search(&mut search_state, &game_state)
}

#[derive(Parser, Debug)]
struct TreePerfCliArgs {
    #[arg(short = 's', long, default_value_t = 0)]
    scenario: usize,
}

fn main() {
    let args = TreePerfCliArgs::parse();
    println!("Running Scenario {}", args.scenario);

    let mut tt = TranspositionTable::new();
    for _ in 0..5 {
        let now = Instant::now();
        let res = test(&mut tt, args.scenario);
        let end = Instant::now();

        let duration = end - now;
        println!("Took {:.4}s", duration.as_secs_f32());
        println!("{:?}", res);
        println!("{:?}", tt);
        tt.reset();
    }
}

// cargo run -p santorini_core --release --bin tree_perf -- -s 0
// sudo sysctl kernel.perf_event_paranoid=1
// RUSTFLAGS="-C force-frame-pointers=yes -C symbol-mangling-version=v0 -C target-cpu=native" cargo flamegraph -p santorini_core --bin tree_perf --release -- -s 0
