use std::time::Instant;

use santorini_core::{
    board::{BoardState, FullGameState},
    gods::{StateOnlyMapper, mortal::mortal_next_states},
};

fn main() {
    for _ in 0..10 {
        run_single_test();
    }
}

fn run_single_test() {
    let state_str = "0000000000000000000000000/1/mortal:11,13/mortal:7,17";
    let state = FullGameState::try_from(state_str).unwrap();

    let now = Instant::now();
    let result_count = _test_depth(&state.board, 5);
    let duration = now.elapsed();
    let per_sec = result_count as f32 / duration.as_secs_f32();
    println!(
        "Found {} children. Took {:.4}s. Results/sec: {:.4}",
        result_count,
        duration.as_secs_f32(),
        per_sec
    );
}

fn _test_depth(state: &BoardState, depth: usize) -> usize {
    let children =
        mortal_next_states::<BoardState, StateOnlyMapper, false>(state, state.current_player);

    if depth == 1 {
        children.len()
    } else {
        children.iter().map(|c| _test_depth(c, depth - 1)).sum()
    }
}

// RUSTFLAGS='-C target-cpu=native' cargo run -p santorini_core  --bin perft --release
// cargo flamegraph -p santorini_core  --bin perft --release
// sudo sysctl kernel.perf_event_paranoid=1
// CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph -p santorini_core  --bin perft --release
