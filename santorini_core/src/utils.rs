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

pub const fn grid_position_builder<T: Copy>(
    outer_corner: T,
    outer_edge: T,
    outer_mid: T,
    inner_corner: T,
    inner_mid: T,
    center: T,
    ) -> [T; 25] {
    [
        outer_corner, outer_edge, outer_mid, outer_edge, outer_corner,
        outer_edge, inner_corner, inner_mid, inner_corner, outer_edge,
        outer_mid, inner_mid, center, inner_mid, outer_mid,
        outer_edge, inner_corner, inner_mid, inner_corner, outer_edge,
        outer_corner, outer_edge, outer_mid, outer_edge, outer_corner,
    ]
}


#[cfg(test)]
mod tests {
    use crate::board::FullGameState;

    use super::*;

    #[test]
    fn test_grid_position_builder() {
        let result = grid_position_builder(1,2,3,4,5,6);
        let expected = [
            1, 2, 3, 2, 1,
            2, 4, 5, 4, 2,
            3, 5, 6, 5, 3,
            2, 4, 5, 4, 2,
            1, 2, 3, 2, 1,
        ];

        assert_eq!(result, expected);
    }
}
