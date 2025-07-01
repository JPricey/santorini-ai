#![feature(portable_simd)]

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
pub mod move_container;
pub mod random_utils;

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

#[cfg(test)]
mod tests {
}
