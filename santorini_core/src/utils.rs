#![allow(unused)]
use crate::board::{BitmapType, Coord, BOARD_WIDTH};

pub fn coord_to_position(coord: Coord) -> usize {
    coord.x + coord.y * BOARD_WIDTH
}

pub fn print_full_bitmap(mut mask: BitmapType) {
    for _ in 0..5 {
        let lower = mask & 0b11111;
        let output = format!("{:05b}", lower);
        eprintln!("{}", output.chars().rev().collect::<String>());
        mask = mask >> 5;
    }
}

