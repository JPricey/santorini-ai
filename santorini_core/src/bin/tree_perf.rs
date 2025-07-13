use clap::Parser;
use std::time::Instant;

use santorini_core::{
    board::FullGameState,
    search::{MaxDepthStaticSearchTerminator, SearchContext, SearchState, negamax_search},
    transposition_table::TranspositionTable,
};

fn test(tt: &mut TranspositionTable, scenario: usize) -> SearchState {
    let mut search_state = SearchContext::new(tt);
    match scenario {
        0 => {
            // Starting position
            let state =
                FullGameState::try_from("0000000000000000000000000/1/mortal:2,13/mortal:7,20")
                    .unwrap();
            negamax_search::<MaxDepthStaticSearchTerminator<8>>(&mut search_state, &state)
        }
        1 => {
            // Starting position
            let state =
                FullGameState::try_from("0000002100040001111021200/1/mortal:7,16/mortal:17,21")
                    .unwrap();
            negamax_search::<MaxDepthStaticSearchTerminator<8>>(&mut search_state, &state)
        }
        2 => {
            // Starting position
            let state =
                FullGameState::try_from("0000011000020004003011112/2/mortal:21,23/mortal:11,16")
                    .unwrap();
            negamax_search::<MaxDepthStaticSearchTerminator<12>>(&mut search_state, &state)
        }
        _ => panic!("Unknown scenario"),
    }
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
