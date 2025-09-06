#![feature(portable_simd)]

pub mod bitboard;
pub mod board;
pub mod consistency_checker;
pub mod direction;
pub mod engine;
pub mod fen;
pub mod gods;
pub mod hashing;
pub mod matchup;
pub mod move_container;
pub mod move_picker;
pub mod nnue;
pub mod placement;
pub mod player;
pub mod random_utils;
pub mod search;
pub mod search_terminators;
pub mod square;
pub mod transposition_table;
pub mod uci_types;
pub mod utils;

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

pub(crate) use transmute_enum;
pub(crate) use transmute_enum_masked;

#[cfg(test)]
mod tests {}
