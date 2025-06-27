use std::time::Instant;

use santorini_core::{
    board::{BoardState, FullGameState},
    gods::{ALL_GODS_BY_ID, GodName, GodPower},
};

fn main() {
    let depth = 5;
    let god = GodName::Mortal.to_power();
    for _ in 0..5 {
        run_single_test_makemove(depth, god);
    }
}

fn run_single_test_makemove(depth: usize, god: &'static GodPower) {
    let state_str = "0000000000000000000000000/1/mortal:11,13/mortal:7,17";
    let mut state = FullGameState::try_from(state_str).unwrap();

    let now = Instant::now();
    let result_count = _test_depth_makemove(&mut state.board, god, depth);
    let duration = now.elapsed();
    let per_sec = result_count as f32 / duration.as_secs_f32();
    println!(
        "MakeMove  : Found {} children. Took {:.4}s. Results/sec: {:.4}",
        result_count,
        duration.as_secs_f32(),
        per_sec
    );
}

fn _test_depth_makemove(state: &mut BoardState, god: &'static GodPower, depth: usize) -> usize {
    if depth == 0 {
        (state.height_map[0].0 > 0) as usize
    } else {
        let mut sum: usize = 0;
        let actions = (god.get_all_moves)(state, state.current_player);
        for action in actions {
            god.make_move(state, action);
            sum += _test_depth_makemove(state, god, depth - 1);
            god.unmake_move(state, action);
        }
        sum
    }
}

// RUSTFLAGS='-C target-cpu=native' cargo run -p santorini_core  --bin perft --release
// cargo flamegraph -p santorini_core  --bin perft --release
// sudo sysctl kernel.perf_event_paranoid=1
// CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph -p santorini_core  --bin perft --release
