use std::fmt;

use crate::{square::Square, transmute_enum};

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
