pub mod board;
pub mod bitboard;
pub mod square;
pub mod player;
pub mod engine;
pub mod fen;
pub mod gods;
pub mod search;
pub mod transposition_table;
pub mod uci_types;
pub mod utils;
pub mod nnue;

/// Macro used to transmute enums to their binary representation.
/// This is needed to make most enum functions compile-time constants (c++ constexpr).
///
///     x  --> enum value in correct binary representation
///   mask --> bitmask to get only the relevant bits for the representation
///
/// UB: as long as the enum in use is #[repr(mask)] this cannot fail
macro_rules! transmute_enum_masked {
    ($x:expr, $mask:expr) => {
        unsafe { std::mem::transmute($x & $mask) }
    };
}

macro_rules! transmute_enum {
    ($x:expr) => {
        unsafe { std::mem::transmute($x) }
    };
}

pub(crate) use transmute_enum_masked;
pub(crate) use transmute_enum;
