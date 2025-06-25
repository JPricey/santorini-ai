#![allow(unused)]
use crate::{bitboard::BitBoard, board::BOARD_WIDTH};

pub const EXCEPT_LEFT_COL: BitBoard =
    BitBoard(0b11110 | 0b11110 << 5 | 0b11110 << 10 | 0b11110 << 15 | 0b11110 << 20);
pub const EXCEPT_RIGHT_COL: BitBoard =
    BitBoard(0b01111 | 0b01111 << 5 | 0b01111 << 10 | 0b01111 << 15 | 0b01111 << 20);

pub fn move_all_workers_one_include_original_workers(mask: BitBoard) -> BitBoard {
    let down = mask.0 >> BOARD_WIDTH;
    let up = mask.0 << BOARD_WIDTH;
    let verticals = (mask.0 | up | down);

    let left = (verticals >> 1) & EXCEPT_RIGHT_COL.0;
    let right = (verticals << 1) & EXCEPT_LEFT_COL.0;

    BitBoard((verticals | left | right) & BitBoard::MAIN_SECTION_MASK.0)
}

pub fn move_all_workers_one_exclude_original_workers(mask: BitBoard) -> BitBoard {
    return move_all_workers_one_include_original_workers(mask) & !mask;
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
        outer_corner,
        outer_edge,
        outer_mid,
        outer_edge,
        outer_corner,
        outer_edge,
        inner_corner,
        inner_mid,
        inner_corner,
        outer_edge,
        outer_mid,
        inner_mid,
        center,
        inner_mid,
        outer_mid,
        outer_edge,
        inner_corner,
        inner_mid,
        inner_corner,
        outer_edge,
        outer_corner,
        outer_edge,
        outer_mid,
        outer_edge,
        outer_corner,
    ]
}

#[cfg(test)]
mod tests {
    use crate::{board::{FullGameState, NEIGHBOR_MAP}, square::Square};

    use super::*;

    #[test]
    fn test_grid_position_builder() {
        let result = grid_position_builder(1, 2, 3, 4, 5, 6);
        let expected = [
            1, 2, 3, 2, 1, 2, 4, 5, 4, 2, 3, 5, 6, 5, 3, 2, 4, 5, 4, 2, 1, 2, 3, 2, 1,
        ];

        assert_eq!(result, expected);
    }

    #[test]
    fn test_move_all_workers_one_worker() {
        for pos in 0..25 {
            let worker_mask = BitBoard::as_mask_u8(pos);
            let expected = NEIGHBOR_MAP[pos as usize];
            let computed = move_all_workers_one_exclude_original_workers(worker_mask);

            assert_eq!(computed, expected);
        }
    }

    #[test]
    fn test_move_all_workers_two_workers() {
        for p1 in 0..25 {
            for p2 in 0..25 {
                let mask = BitBoard(1 << p1 | 1 << p2);
                {
                    let expected = (NEIGHBOR_MAP[p1] | NEIGHBOR_MAP[p2]) & !mask;
                    let computed = move_all_workers_one_exclude_original_workers(mask);
                    assert_eq!(computed, expected);
                }

                {
                    let expected = (NEIGHBOR_MAP[p1] | NEIGHBOR_MAP[p2]) | mask;
                    let computed = move_all_workers_one_include_original_workers(mask);
                    assert_eq!(computed, expected);
                }
            }
        }
    }
}
