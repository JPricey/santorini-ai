use const_for::const_for;
use std::{fmt, ops::Mul};

use crate::{
    direction::{Direction, ICoord},
    square::Square,
    transmute_enum,
};

pub const BOARD_WIDTH: usize = 5;
pub const NUM_SQUARES: usize = BOARD_WIDTH * BOARD_WIDTH;

pub type BitboardMapping = [BitBoard; NUM_SQUARES];

#[macro_export]
macro_rules! for_each_direction {
    ($dir: ident => $body: block) => {
        use const_for::const_for;
        const_for!(i in 0..8 => {
            let $dir = $crate::direction::Direction::from_u8(i);
            $body
        })
    }
}

#[macro_export]
macro_rules! square_map {
    ($square: ident => $body: expr) => {{
        let mut arr: [core::mem::MaybeUninit<_>; NUM_SQUARES] =
            unsafe { core::mem::MaybeUninit::uninit().assume_init() };
        let mut i = 0;
        while i < NUM_SQUARES {
            let $square: Square = $crate::transmute_enum!(i as u8);
            arr[i] = core::mem::MaybeUninit::new($body);
            i += 1;
        }
        unsafe { std::mem::transmute_copy::<_, [_; NUM_SQUARES]>(&arr) }
    }};
}

pub const NEIGHBOR_MAP: BitboardMapping = square_map!(square => {
    let mut res = BitBoard::EMPTY;
    let coord = square.to_icoord();
    for_each_direction!(dir => {
        let new_coord = coord.add(dir.to_icoord());
        if let Some(n) = new_coord.to_square() {
            res = res.bit_or(BitBoard::as_mask(n));
        }
    });
    res
});

pub const INCLUSIVE_NEIGHBOR_MAP: BitboardMapping = square_map!(square => {
    let coord = square.to_icoord();
    let mut res = BitBoard::as_mask(square);
    for_each_direction!(dir => {
        let new_coord = coord.add(dir.to_icoord());
        if let Some(n) = new_coord.to_square() {
            res = res.bit_or(BitBoard::as_mask(n));
        }
    });
    res
});

pub const WRAPPING_NEIGHBOR_MAP: BitboardMapping = square_map!(square => {
    let coord = square.to_icoord();
    let mut res = BitBoard::EMPTY;
    for_each_direction!(dir => {
        let mut new_coord = coord.add(dir.to_icoord()).add(ICoord::new(5, 5));
        new_coord.col %= 5;
        new_coord.row %= 5;

        res = res.bit_or(BitBoard::as_mask(new_coord.to_square().unwrap()));
    });
    res
});

pub const PUSH_MAPPING: [[Option<Square>; NUM_SQUARES]; NUM_SQUARES] = {
    let mut result = [[None; NUM_SQUARES]; NUM_SQUARES];
    const_for!(from in 0..25 => {
        const_for!(to in 0..25 => {
            let to_mask = BitBoard::as_mask(transmute_enum!(to as u8));
            if (NEIGHBOR_MAP[from as usize].0 & to_mask.0) != 0 {
                let delta = to - from;
                let dest = to + delta;
                if dest >= 0 && dest < 25 {
                    if NEIGHBOR_MAP[to as usize].0 & 1 << dest != 0 {
                        result[from as usize][to as usize] = Some(transmute_enum!(dest as u8));
                    }
                }
            }
        });
    });
    result
};

pub const DIRECTION_MAPPING: [[Option<Square>; NUM_SQUARES]; 8] = {
    let mut result = [[None; NUM_SQUARES]; 8];

    const_for!(direction_idx in 0..8 => {
        let direction = Direction::from_u8(direction_idx as u8);
        const_for!(from in 0..25 => {
            let from_square: Square = transmute_enum!(from as u8);
            let from_coord = from_square.to_icoord();
            let delta = direction.to_icoord();
            let to_coord = from_coord.add(delta);
            if let Some(to_square) = to_coord.to_square() {
                result[direction_idx][from as usize] = Some(to_square);
            }
        });
    });

    result
};

pub const WRAPPING_DIRECTION_MAPPING: [[Square; NUM_SQUARES]; 8] = {
    let mut result = [[Square::A1; NUM_SQUARES]; 8];

    const_for!(direction_idx in 0..8 => {
        let direction = Direction::from_u8(direction_idx as u8);
        const_for!(from in 0..25 => {
            let from_square: Square = transmute_enum!(from as u8);
            let from_coord = from_square.to_icoord();
            let delta = direction.to_icoord();
            let to_coord = from_coord.add(delta).wrap_in_bounds();
            result[direction_idx][from as usize] = to_coord.to_square().unwrap();
        });
    });

    result
};

pub const WIND_AWARE_NEIGHBOR_MAP: [BitboardMapping; 9] = {
    let mut result = [[BitBoard::EMPTY; NUM_SQUARES]; 9];

    result[0] = NEIGHBOR_MAP;

    const_for!(direction_idx in 0..8 => {
        const_for!(square_idx in 0..25 => {
            if let Some(wind_square) = DIRECTION_MAPPING[direction_idx][square_idx] {
                result[direction_idx + 1][square_idx] = NEIGHBOR_MAP[square_idx].bit_and(BitBoard::as_mask(wind_square).bit_not());
            } else {
                result[direction_idx + 1][square_idx] = NEIGHBOR_MAP[square_idx]
            }
        });
    });

    result
};

pub const WIND_AWARE_INCLUSIVE_NEIGHBOR_MAP: [BitboardMapping; 9] = {
    let mut result = [[BitBoard::EMPTY; NUM_SQUARES]; 9];

    result[0] = INCLUSIVE_NEIGHBOR_MAP;

    const_for!(direction_idx in 0..8 => {
        const_for!(square_idx in 0..25 => {
            if let Some(wind_square) = DIRECTION_MAPPING[direction_idx][square_idx] {
                result[direction_idx + 1][square_idx] = INCLUSIVE_NEIGHBOR_MAP[square_idx].bit_and(BitBoard::as_mask(wind_square).bit_not());
            } else {
                result[direction_idx + 1][square_idx] = INCLUSIVE_NEIGHBOR_MAP[square_idx]
            }
        });
    });

    result
};

pub const WIND_AWARE_WRAPPING_NEIGHBOR_MAP: [BitboardMapping; 9] = {
    let mut result = [[BitBoard::EMPTY; NUM_SQUARES]; 9];

    result[0] = WRAPPING_NEIGHBOR_MAP;

    const_for!(direction_idx in 0..8 => {
        result[direction_idx + 1] = square_map!(square => {
            let coord = square.to_icoord();
            let mut res = BitBoard::EMPTY;
            for_each_direction!(dir => {
                if dir as usize != direction_idx {
                    let mut new_coord = coord.add(dir.to_icoord()).add(ICoord::new(5, 5));
                    new_coord.col %= 5;
                    new_coord.row %= 5;

                    res = res.bit_or(BitBoard::as_mask(new_coord.to_square().unwrap()));
                }
            });
            res
        });
    });

    result
};

pub const MIDDLE_SPACES_MASK: BitBoard = BitBoard(0b00000_01110_01110_01110_00000);
pub const PERIMETER_SPACES_MASK: BitBoard = MIDDLE_SPACES_MASK
    .bit_not()
    .bit_and(BitBoard::MAIN_SECTION_MASK);

pub(crate) fn apply_mapping_to_mask(mask: BitBoard, mapping: &BitboardMapping) -> BitBoard {
    mask.into_iter()
        .fold(BitBoard::EMPTY, |accum: BitBoard, s: Square| {
            accum | mapping[s as usize]
        })
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default, Hash)]
pub struct BitBoard(pub u32);

impl Ord for BitBoard {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for BitBoard {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Idea for ops implementation is from https://github.com/analog-hors/tantabus
/// Implement math standard operations
macro_rules! impl_math_ops {
    ($($trait:ident::$fn:ident),*) => {
        $(impl std::ops::$trait for BitBoard {
            type Output = Self;

            fn $fn(self, other: Self) -> Self::Output {
                Self(std::ops::$trait::$fn(self.0, other.0))
            }
        })*
    };
}

impl_math_ops! {
    Shr::shr,
    Shl::shl,
    BitAnd::bitand,
    BitOr::bitor,
    BitXor::bitxor
}

/// Implement math assignment operations
macro_rules! impl_math_assign_ops {
    ($($trait:ident::$fn:ident),*) => {
        $(impl std::ops::$trait for BitBoard {

            fn $fn(&mut self, other: Self) {
                std::ops::$trait::$fn(&mut self.0, other.0)
            }
        })*
    };
}

impl_math_assign_ops! {
    ShrAssign::shr_assign,
    ShlAssign::shl_assign,
    BitAndAssign::bitand_assign,
    BitOrAssign::bitor_assign,
    BitXorAssign::bitxor_assign
}

impl std::ops::Not for BitBoard {
    type Output = Self;

    fn not(self) -> Self::Output {
        self.bit_not()
    }
}

impl fmt::Display for BitBoard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = format!("\n      Bitboard: {}\n", self.0);

        for square in 0_usize..25 {
            if square % 5 == 0 {
                s.push_str(format!("\n{}   ", (5 - square / 5)).as_str())
            }

            if self.get_bit(Square::from(square)) {
                s.push_str("X ");
            } else {
                s.push_str("- ");
            }
        }
        s.push_str("\n\n    A B C D E");
        write!(f, "{s}")
    }
}

impl BitBoard {
    pub const EMPTY: Self = Self(0);
    pub const MAIN_SECTION_MASK: Self = Self((1 << 25) - 1);
    pub const OFF_SECTION_MASK: Self = Self(!Self::MAIN_SECTION_MASK.0);

    pub const CONDITIONAL_MASK: [Self; 2] = [Self::EMPTY, Self::MAIN_SECTION_MASK];

    pub const fn as_mask(square: Square) -> Self {
        let data = 1u32 << square as u8;
        Self(data)
    }

    pub const fn as_mask_u8(square: u8) -> Self {
        Self(1 << square)
    }

    pub const fn get_bit(self, square: Square) -> bool {
        self.get_bit_masked(1 << square as u8)
    }

    pub const fn get_bit_masked(self, mask: u32) -> bool {
        self.0 & mask != 0
    }

    pub const fn lsb(self) -> Square {
        transmute_enum!(self.0.trailing_zeros() as u8)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn is_not_empty(self) -> bool {
        self.0 != 0
    }

    pub const fn count_ones(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn trailing_zeros(self) -> u32 {
        self.0.trailing_zeros()
    }

    pub fn all_squares(&self) -> Vec<Square> {
        let mut res = Vec::with_capacity(self.count_ones() as usize);
        for square in *self {
            res.push(square);
        }
        res
    }

    pub fn contains_square(self, square: Square) -> bool {
        (self & BitBoard::as_mask(square)).is_not_empty()
    }

    // const bit operations, since the trait is non-const
    pub const fn bit_and(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn bit_or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn bit_not(self) -> Self {
        Self(!self.0)
    }

    pub fn flip_vertical(self) -> BitBoard {
        let mut board = _delta_swap(self, 0b11111, 20);
        board = _delta_swap(board, 0b1111100000, 10);
        board
    }

    pub fn flip_horizontal(self) -> BitBoard {
        let mut board = _delta_swap(self, 0b00001_00001_00001_00001_00001, 4);
        board = _delta_swap(board, 0b00010_00010_00010_00010_00010, 2);
        board
    }

    /// "Transpose" is to flip across the A5 -> E1 diagonal
    pub fn flip_transpose(self) -> BitBoard {
        // https://stackoverflow.com/questions/72097570/rotate-and-reflect-a-5x5-bitboard
        let mut board = _delta_swap(self, 0x00006300, 16);
        board = _delta_swap(board, 0x020a080a, 4);
        board = _delta_swap(board, 0x0063008c, 8);
        board = _delta_swap(board, 0x00006310, 16);
        board
    }
}

fn _delta_swap(board: BitBoard, mask: u32, shift: u32) -> BitBoard {
    let delta = ((board.0 >> shift) ^ board.0) & mask;
    BitBoard((board.0 ^ delta) ^ (delta << shift))
}

impl Iterator for BitBoard {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        if *self == Self::EMPTY {
            None
        } else {
            let sq = self.lsb();
            self.0 &= self.0 - 1;

            Some(sq)
        }
    }
}

pub trait BitboardOps {
    fn and(self, other: BitBoard) -> BitBoard;
}

impl BitboardOps for BitBoard {
    fn and(self, other: BitBoard) -> BitBoard {
        self & other
    }
}

pub struct PanicBitboard {}

impl BitboardOps for PanicBitboard {
    fn and(self, _other: BitBoard) -> BitBoard {
        unreachable!()
    }
}

impl Mul<u32> for BitBoard {
    type Output = Self;

    fn mul(self, rhs: u32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

#[cfg(test)]
mod tests {
    use crate::square::Square;

    use super::*;

    #[test]
    fn test_flip_board_v() {
        for b in 0..25 {
            let board: BitBoard = BitBoard(1 << b);
            let row = b / 5;
            let col = b % 5;

            let flipped = board.flip_vertical();
            let pos = flipped.0.trailing_zeros();
            let arow = pos / 5;
            let acol = pos % 5;

            assert_eq!(arow, 4 - row);
            assert_eq!(acol, col);
        }
    }

    #[test]
    fn test_flip_board_h() {
        for b in 0..25 {
            let board = BitBoard(1 << b);
            let row = b / 5;
            let col = b % 5;

            let flipped = board.flip_horizontal();
            let pos = flipped.trailing_zeros();
            let arow = pos / 5;
            let acol = pos % 5;

            assert_eq!(arow, row);
            assert_eq!(acol, 4 - col);
        }
    }

    #[test]
    fn test_transpose() {
        for b in 0..25 {
            let board = BitBoard(1 << b);
            let row = b / 5;
            let col = b % 5;

            let flipped = board.flip_transpose();

            let pos = flipped.trailing_zeros();
            let arow = pos / 5;
            let acol = pos % 5;

            assert_eq!(row, acol);
            assert_eq!(col, arow);

            // eprintln!("board: {board}");
            // eprintln!("flipped: {flipped}");
        }
    }
}
