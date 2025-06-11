#![allow(unused)]
use santorini_ai::board::{
    BOARD_WIDTH, BitmapType, Coord, NUM_SQUARES, SantoriniState, coord_to_position,
    position_to_coord,
};
use std::hint::black_box;

fn output_neighbor_mask() {
    for p in 0..NUM_SQUARES {
        let coord = position_to_coord(p);
        let (x, y) = (coord.x as i64, coord.y as i64);

        let mut neighbor_mask = 0 as BitmapType;
        for dx in [-1, 0, 1] {
            for dy in [-1, 0, 1] {
                if dx == dy && dx == 0 {
                    continue;
                }

                let nx = x + dx;
                let ny = y + dy;

                if nx < 0 || nx >= BOARD_WIDTH as i64 || ny < 0 || ny >= BOARD_WIDTH as i64 {
                    continue;
                }

                let nc: usize = coord_to_position(Coord::new(nx as usize, ny as usize));
                neighbor_mask |= 1 << nc;
            }
        }
        println!("{},", neighbor_mask);

        // println!("{:?}", coord);
        // print_full_bitmap(neighbor_mask);
    }
}

fn benchmark_finding_children_with_hueristic() {
    let state = SantoriniState::new_basic_state();
    let start_time = std::time::Instant::now();
    for _ in 0..1000000 {
        black_box(state.get_next_states_with_scores());
    }
    let elapsed = start_time.elapsed();
    println!("v2: {} ms", elapsed.as_millis());
}

fn benchmark_finding_children_fast() {
    let state = SantoriniState::new_basic_state();
    let start_time = std::time::Instant::now();
    for _ in 0..1000000 {
        black_box(state.get_valid_next_states());
    }
    let elapsed = start_time.elapsed();
    println!("fast: {} ms", elapsed.as_millis());
}

fn benchmark_finding_children_interactive() {
    let state = SantoriniState::new_basic_state();
    let start_time = std::time::Instant::now();
    for _ in 0..1000000 {
        black_box(state.get_next_states_interactive());
    }
    let elapsed = start_time.elapsed();
    println!("interactive: {} ms", elapsed.as_millis());
}

fn main() {
    println!("Hello world")
}
