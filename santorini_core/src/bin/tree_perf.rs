use std::time::Instant;

use santorini_core::{
    board::FullGameState,
    search::{MaxDepthStaticSearchTerminator, SearchContext, search_with_state},
    transposition_table::TranspositionTable,
};

fn main() {
    let state_str = "0000000000000000000000000/1/mortal:2,13/mortal:7,20";
    // let state_str = "0000002100040001111021200/1/mortal:7,16/mortal:17,21";

    let state = FullGameState::try_from(state_str).unwrap();

    let mut tt = TranspositionTable::new();
    for _ in 0..2 {
        let mut search_state = SearchContext::new(&mut tt);

        let now = Instant::now();
        search_with_state::<MaxDepthStaticSearchTerminator<7>>(&mut search_state, &state);
        let end = Instant::now();

        let duration = end - now;
        println!("Took {:.4}s", duration.as_secs_f32());
        println!("{:?}", tt);
        tt.reset();
    }
}

// cargo run -p santorini_core --release --bin tree_perf
// sudo sysctl kernel.perf_event_paranoid=1
// RUSTFLAGS="-C force-frame-pointers=yes -C symbol-mangling-version=v0 -C target-cpu=native" cargo flamegraph -p santorini_core --bin tree_perf --release
