use clap::Parser;
use std::time::Instant;

use santorini_core::{
    board::FullGameState,
    search::{SearchContext, SearchState, negamax_search},
    search_terminators::DynamicMaxDepthSearchTerminator,
    transposition_table::TranspositionTable,
};

const SCENARIOS: [(&'static str, usize); 3] = [
    ("0000000000000000000000000/1/mortal:2,13/mortal:7,20", 8),
    ("0000002100040001111021200/1/mortal:7,16/mortal:17,21", 14),
    ("0000011000020004003011112/2/mortal:21,23/mortal:11,16", 15),
];

fn test(tt: &mut TranspositionTable, scenario: usize) -> SearchState {
    let (state_str, depth) = SCENARIOS[scenario];
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
