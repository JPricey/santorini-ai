#![feature(stdarch_x86_avx512)]
#![feature(avx512_target_feature)]
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

#[rustfmt::skip]
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
    use crate::{
        board::{FullGameState, NEIGHBOR_MAP},
        square::Square,
    };

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
            let expected = NEIGHBOR_MAP[pos as usize] | worker_mask;
            let computed = move_all_workers_one_include_original_workers(worker_mask);

            assert_eq!(computed, expected);
        }
    }

    #[test]
    fn test_move_all_workers_two_workers() {
        for p1 in 0..25 {
            for p2 in 0..25 {
                let mask = BitBoard(1 << p1 | 1 << p2);
                let expected = (NEIGHBOR_MAP[p1] | NEIGHBOR_MAP[p2]) | mask;
                let computed = move_all_workers_one_include_original_workers(mask);
                assert_eq!(computed, expected);
            }
        }
    }
}

pub fn print_cpu_arch() {
    println!("Target arch: {}", std::env::consts::ARCH);
    println!("Target family: {}", std::env::consts::FAMILY);
    println!("Target os: {}", std::env::consts::OS);

    #[cfg(target_arch = "x86_64")]
    {
        println!("sse2: {}", std::is_x86_feature_detected!("sse2"));
        println!("avx: {}", std::is_x86_feature_detected!("avx"));
        println!("avx512f: {}", std::is_x86_feature_detected!("avx512f"));
        println!("avx2: {}", std::is_x86_feature_detected!("avx2"));
        println!("fma: {}", std::is_x86_feature_detected!("fma"));
        println!("bmi2: {}", std::is_x86_feature_detected!("bmi2"));
    }

    #[cfg(target_feature = "avx2")]
    {
        use std::arch::x86_64::*;
        println!("using avx2");
    }
}

/// data for move testing
pub const SEARCH_TEST_SCENARIOS: [(&'static str, usize); 56] = [
    ("0000000000000000000000000/1/mortal:2,13/mortal:7,20", 9),
    ("0000002100040001111021200/1/mortal:7,16/mortal:17,21", 16),
    ("0000011000020004003011112/2/mortal:21,23/mortal:11,16", 18),
    ("3444441122104224302401000/1/mortal:A3,D3/mortal:B3,E4", 6),
    ("3444431122104224202401000/1/mortal:B2,D3/mortal:B4,E4", 15),
    ("2444431122104224201401000/1/mortal:B2,E3/mortal:A3,E4", 20),
    ("2444431122104222201401000/1/mortal:A3,E3/mortal:B4,E4", 24),
    ("2444431122104122200401000/1/mortal:A3,E4/mortal:B4,D4", 22),
    ("2443431112104122200401000/1/mortal:A3,D4/mortal:B4,E3", 24),
    ("2442431111104122200401000/1/mortal:A3,E3/mortal:B4,D3", 20),
    ("2442431111104122200201000/1/mortal:A3,E4/mortal:B4,D4", 21),
    ("2442331111104112200201000/1/mortal:A3,E3/mortal:B4,C4", 20),
    ("2441331111104112200101000/1/mortal:A3,D3/mortal:B4,D4", 16),
    ("2441331011104112200001000/1/mortal:A3,D4/mortal:B4,E4", 16),
    ("2441321011104012200001000/1/mortal:A4,D4/mortal:B4,D5", 16),
    ("2331321011104012200001000/1/mortal:A3,D4/mortal:A5,D5", 15),
    ("2331311011104012100001000/1/mortal:A4,D4/mortal:B4,D5", 15),
    ("1331311011103012100001000/1/mortal:B4,D4/mortal:A3,D5", 13),
    ("1311311011103012100001000/1/mortal:A4,D4/mortal:A3,E4", 14),
    ("0444433112410421424104011/2/mortal:A2,E2/mortal:B3,E4", 7),
    ("0444433112310411424104000/2/mortal:A2,E2/mortal:C2,E3", 12),
    ("0444433112310411424102000/2/mortal:B3,E2/mortal:B1,E3", 14),
    ("0444433112310411314102000/2/mortal:A2,E2/mortal:C2,E3", 16),
    ("0444433102310411314101000/2/mortal:B3,E2/mortal:C2,E4", 18),
    ("0444433102310211314101000/2/mortal:B3,E3/mortal:C2,D3", 19),
    ("0444433102310211214001000/2/mortal:B3,E4/mortal:C3,D3", 24),
    ("0444432102310211114001000/2/mortal:C4,E4/mortal:D3,D4", 17),
    ("0443432102300211114001000/2/mortal:C3,E4/mortal:C4,D3", 19),
    ("0342432102300211114001000/2/mortal:C3,D5/mortal:B3,D3", 20),
    ("0342432102300211112001000/2/mortal:C4,D5/mortal:B3,E4", 21),
    ("0342431102300111112001000/2/mortal:B4,D5/mortal:B3,D3", 19),
    ("0342431101300111102001000/2/mortal:B4,E4/mortal:B3,E3", 15),
    ("0342431001300111002001000/2/mortal:C4,E4/mortal:B4,E3", 12),
    ("0342431001300011001001000/2/mortal:C4,D5/mortal:B4,D4", 12),
    ("0311311011103012000001000/1/mortal:A3,D4/mortal:B4,E4", 12),
    ("0311211011103002000001000/1/mortal:A3,E5/mortal:B4,D5", 12),
    ("0310211010103002000001000/1/mortal:A3,D4/mortal:B4,C5", 13),
    ("0310201010102002000001000/1/mortal:B2,D4/mortal:B3,C5", 13),
    ("0310200010101002000001000/1/mortal:B3,D4/mortal:A3,C5", 11),
    ("0310200010100001000001000/1/mortal:A3,D4/mortal:B2,C5", 11),
    ("0310100010100001000000000/1/mortal:A3,C4/mortal:C3,C5", 10),
    ("0242331001300011001001000/2/mortal:B3,D5/mortal:B4,C4", 14),
    ("0210100000100001000000000/1/mortal:A3,D4/mortal:C2,C5", 11),
    ("0142231001300011001001000/2/mortal:B3,E5/mortal:B4,C3", 16),
    ("0142211001300011001001000/2/mortal:C2,E5/mortal:B5,C3", 16),
    ("0141201001300011001001000/2/mortal:C2,D5/mortal:C3,C4", 12),
    ("0141100001300011001001000/2/mortal:C2,E4/mortal:C4,D4", 11),
    ("0141000001300011001000000/2/mortal:B3,E4/mortal:C3,C4", 10),
    ("0131000001300001001000000/2/mortal:B3,D4/mortal:B4,C3", 10),
    ("0130000001200001001000000/2/mortal:B3,C4/mortal:B5,C3", 10),
    ("0110000001200001001000000/2/mortal:B3,D4/mortal:C3,C4", 10),
    ("0110000001100001000000000/2/mortal:A3,D4/mortal:C2,C4", 10),
    ("0110000000100001000000000/1/mortal:A3,D3/mortal:C2,C4", 10),
    ("0010000000100001000000000/2/mortal:A3,D3/mortal:B4,C2", 9),
    ("0010000000100000000000000/1/mortal:B3,D3/mortal:B4,C2", 8),
    ("0000000000100000000000000/2/mortal:B3,D3/mortal:C2,C4", 9),
];
