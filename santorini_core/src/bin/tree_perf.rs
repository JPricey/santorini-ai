use std::time::Instant;

use santorini_core::{
    board::FullGameState,
    search::{MaxDepthStaticSearchTerminator, SearchContext, search_with_state},
    transposition_table::TranspositionTable,
};

// Starting position
const STATE_STR: &str = "0000000000000000000000000/1/mortal:2,13/mortal:7,20";
const DEPTH: usize = 7;

// Midgame
// const STATE_STR: &str = "0000002100040001111021200/1/mortal:7,16/mortal:17,21";
// const DEPTH: usize = 8;

// Very uneven
// const STATE_STR: &str = "0000011000020004003011112/2/mortal:21,23/mortal:11,16";
// const DEPTH: usize = 9;

fn main() {
    let state = FullGameState::try_from(STATE_STR).unwrap();

    let mut tt = TranspositionTable::new();
    for _ in 0..5 {
        let mut search_state = SearchContext::new(&mut tt);

        let now = Instant::now();
        let res =
            search_with_state::<MaxDepthStaticSearchTerminator<DEPTH>>(&mut search_state, &state);
        let end = Instant::now();

        let duration = end - now;
        println!("Took {:.4}s", duration.as_secs_f32());
        println!("{:?}", res);
        println!("{:?}", tt);
        tt.reset();
    }
}

// cargo run -p santorini_core --release --bin tree_perf
// sudo sysctl kernel.perf_event_paranoid=1
// RUSTFLAGS="-C force-frame-pointers=yes -C symbol-mangling-version=v0 -C target-cpu=native" cargo flamegraph -p santorini_core --bin tree_perf --release
