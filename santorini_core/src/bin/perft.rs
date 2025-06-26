use std::time::Instant;

use santorini_core::{
    board::{BoardState, FullGameState},
    gods::{
        StateOnlyMapper,
        generic::{make_move, mortal_move_gen, unmake_move},
        mortal::mortal_next_states,
    },
};

fn main() {
    let depth = 5;
    for _ in 0..5 {
        run_single_test(depth);
        run_single_test_makemove(depth);
    }
}

fn run_single_test_makemove(depth: usize) {
    let state_str = "0000000000000000000000000/1/mortal:11,13/mortal:7,17";
    let mut state = FullGameState::try_from(state_str).unwrap();

    let now = Instant::now();
    let result_count =
        _test_depth_makemove(&mut state.board, depth);
    let duration = now.elapsed();
    let per_sec = result_count as f32 / duration.as_secs_f32();
    println!(
        "MakeMove  : Found {} children. Took {:.4}s. Results/sec: {:.4}",
        result_count,
        duration.as_secs_f32(),
        per_sec
    );
}

fn _test_depth_makemove(state: &mut BoardState, depth: usize) -> usize {
    if depth == 0 {
        (state.height_map[0].0 > 0) as usize
    } else {
        let mut sum: usize = 0;
        let actions = mortal_move_gen::<0>(state, state.current_player);
        for action in actions {
            make_move(state, action);
            sum += _test_depth_makemove(state, depth - 1);
            unmake_move(state, action);
        }
        sum
    }
}

fn run_single_test(depth: usize) {
    let state_str = "0000000000000000000000000/1/mortal:11,13/mortal:7,17";
    let state = FullGameState::try_from(state_str).unwrap();

    let now = Instant::now();
    let result_count = _test_depth(&state.board, depth);
    let duration = now.elapsed();
    let per_sec = result_count as f32 / duration.as_secs_f32();
    println!(
        "FullStates: Found {} children. Took {:.4}s. Results/sec: {:.4}",
        result_count,
        duration.as_secs_f32(),
        per_sec
    );
}

fn _test_depth(state: &BoardState, depth: usize) -> usize {
    if depth == 0 {
        (state.height_map[0].0 > 0) as usize
    } else {
        let children =
            mortal_next_states::<BoardState, StateOnlyMapper, false>(state, state.current_player);
        children.iter().map(|c| _test_depth(c, depth - 1)).sum()
    }
}

// RUSTFLAGS='-C target-cpu=native' cargo run -p santorini_core  --bin perft --release
// cargo flamegraph -p santorini_core  --bin perft --release
// sudo sysctl kernel.perf_event_paranoid=1
// CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph -p santorini_core  --bin perft --release
