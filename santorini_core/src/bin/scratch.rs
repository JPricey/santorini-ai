#![allow(dead_code, unused_imports)]
#![feature(portable_simd)]

use rand::{Rng, rng};
use santorini_core::fen::parse_fen;

fn print_hashing_randoms(size: usize) {
    let mut rng = rng();
    let random_numbers = (0..size)
        .map(|_| rng.random_range(0..u64::MAX))
        .collect::<Vec<_>>();

    eprintln!("{:?}", random_numbers);
}

fn debug() {
    let mut total = 0;
    let state = parse_fen("0010000000100000000000000/1/atlas:B3,D3/atlas:B4,C2").unwrap();
    let children = state.get_next_states();
    for child in children {
        let children = child.get_next_states().len();
        total += children;
        println!("{:?}: {}", child, children);
    }
    println!("Total: {}", total);
}

fn main() {
    debug();
    // print_hashing_randoms(2);
}
