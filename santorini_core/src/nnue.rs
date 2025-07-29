use std::{
    fmt::Debug,
    mem,
    ops::{Deref, DerefMut},
    simd::{Simd, cmp::SimdOrd, num::SimdInt},
};

use crate::{
    bitboard::BitBoard, board::BoardState, gods::GodName, player::Player, search::Hueristic,
};

pub const QA: i32 = 255;
pub const QB: i32 = 64;

pub const SCALE: i32 = 400;

pub const EVAL_LANES: usize = 64;
pub const FEATURE_LANES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[repr(C, align(64))]
pub struct Align64<T>(pub T);

impl<T, const SIZE: usize> Deref for Align64<[T; SIZE]> {
    type Target = [T; SIZE];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T, const SIZE: usize> DerefMut for Align64<[T; SIZE]> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Accumulator {
    pub vals: Align64<[i16; HIDDEN_SIZE]>,
}

#[repr(C)]
pub struct Network {
    pub feature_weights: [Accumulator; TOTAL_FEATURES],
    pub feature_bias: Accumulator,
    pub output_weights: [i16; HIDDEN_SIZE],
    pub output_bias: i16,
}

const BOARD_FEATURES: usize = 125;
const SIDE_WORKER_FEATURES: usize = 25 * 4;
const GOD_COUNT: usize = 11;
const PER_SIDE_FEATURES: usize = GOD_COUNT + SIDE_WORKER_FEATURES;

const ACTIVE_PLAYER_OFFSET: usize = BOARD_FEATURES;
const ACTIVE_PLAYER_WORKER_OFFSET: usize = BOARD_FEATURES + GOD_COUNT;

const OPPO_OFFSET: usize = ACTIVE_PLAYER_OFFSET + PER_SIDE_FEATURES;
const OPPO_WORKER_OFFSET: usize = OPPO_OFFSET + GOD_COUNT;

pub const TOTAL_FEATURES: usize = BOARD_FEATURES + PER_SIDE_FEATURES * 2;
pub const HIDDEN_SIZE: usize = 1024;
// TODO: handle athena bit
pub const FEATURE_COUNT: usize = 25 + 3 * 2;

type FeatureType = u16;
type FeatureArray = [u16; FEATURE_COUNT];

pub static MODEL: Network = unsafe {
    mem::transmute(*include_bytes!(
        "../.././models/gods-labeled.bin"
    ))
};

impl Accumulator {
    pub fn new() -> Self {
        MODEL.feature_bias.clone()
    }

    pub fn add_feature(&mut self, feature_idx: usize) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let wts = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[feature_idx].vals[i..i + FEATURE_LANES],
            );
            let sum = acc + wts;
            sum.copy_to_slice(&mut self.vals.0[i..i + FEATURE_LANES]);
        }
    }

    pub fn remove_feature(&mut self, feature_idx: usize) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let wts = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[feature_idx].vals[i..i + FEATURE_LANES],
            );
            let sum = acc - wts;
            sum.copy_to_slice(&mut self.vals.0[i..i + FEATURE_LANES]);
        }
    }

    pub fn add_remove_feature(&mut self, add_idx: usize, sub_idx: usize) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let add = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[add_idx].vals[i..i + FEATURE_LANES],
            );
            let sub = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[sub_idx].vals[i..i + FEATURE_LANES],
            );
            let sum = add - sub + acc;
            sum.copy_to_slice(&mut self.vals.0[i..i + FEATURE_LANES]);
        }
    }
}

// TODO: equality should be for features only
#[derive(Clone, PartialEq, Eq)]
pub struct LabeledAccumulator {
    feature_array: FeatureArray,
    accumulator: Accumulator,
}

impl Debug for LabeledAccumulator {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("LabeledAccumulator")
            .field("feature_array", &self.feature_array)
            .finish()
    }
}

impl LabeledAccumulator {
    pub fn new_from_scratch(board: &BoardState, god1: GodName, god2: GodName) -> Self {
        let feature_array = build_feature_array(board, god1, god2);
        let mut accumulator = Accumulator::new();

        for feature in feature_array {
            accumulator.add_feature(feature as usize);
        }

        LabeledAccumulator {
            feature_array,
            accumulator,
        }
    }

    pub fn replace_features(&mut self, feature_array: FeatureArray) {
        for (current, &new) in self.feature_array.iter_mut().zip(feature_array.iter()) {
            if *current != new {
                self.accumulator
                    .add_remove_feature(new as usize, *current as usize);
                *current = new;
            }
        }
    }

    pub fn replace_from_board(&mut self, board: &BoardState, god1: GodName, god2: GodName) {
        self.replace_features(build_feature_array(board, god1, god2))
    }

    pub fn evaluate(&self) -> Hueristic {
        MODEL.evaluate(&self.accumulator)
    }
}

impl Network {
    pub fn evaluate(&self, us: &Accumulator) -> Hueristic {
        let mut simd_sum = Simd::<i32, EVAL_LANES>::splat(0);

        let min = Simd::splat(0 as i16);
        let max = Simd::splat(QA as i16);

        for i in (0..HIDDEN_SIZE).step_by(EVAL_LANES) {
            let acc = Simd::<i16, EVAL_LANES>::from_slice(&us.vals[i..i + EVAL_LANES])
                .simd_clamp(min, max)
                .cast::<i32>();
            let acc = acc * acc;
            let weights =
                Simd::<i16, EVAL_LANES>::from_slice(&MODEL.output_weights[i..i + EVAL_LANES])
                    .cast::<i32>();

            let prod = acc * weights;
            simd_sum += prod;
        }

        let mut output = simd_sum.reduce_sum();

        output = (output / QA) + MODEL.output_bias as i32;
        // output += MODEL.output_bias as i32;

        output *= SCALE;
        output /= i32::from(QA) * i32::from(QB);

        output as Hueristic
    }
}

pub fn build_feature_array(board: &BoardState, god1: GodName, god2: GodName) -> FeatureArray {
    let mut res = FeatureArray::default();
    for pos in 0..25 {
        res[pos] = (pos * 5) as FeatureType
            + board.get_true_height(BitBoard::as_mask_u8(pos as u8)) as FeatureType;
    }

    let (own_god_idx, other_god_idx) = match board.current_player {
        Player::One => (god1 as usize, god2 as usize),
        Player::Two => (god2 as usize, god1 as usize),
    };

    fn _add_worker_features(
        board: &BoardState,
        worker_map: BitBoard,
        features: &mut FeatureArray,
        feature_offset: FeatureType,
        mut index: usize,
    ) -> usize {
        for pos in worker_map {
            let worker_height = board.get_height_for_worker(BitBoard::as_mask(pos));
            let feature: FeatureType =
                feature_offset + 4 * (pos as FeatureType) + worker_height as FeatureType;
            features[index] = feature;
            index += 1
        }
        index
    }
    let (own_workers, other_workers) = match board.current_player {
        Player::One => (0, 1),
        Player::Two => (1, 0),
    };

    res[25] = (ACTIVE_PLAYER_OFFSET + own_god_idx) as FeatureType;
    _add_worker_features(
        board,
        board.workers[own_workers] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        ACTIVE_PLAYER_WORKER_OFFSET as FeatureType,
        26,
    );

    res[28] = (OPPO_OFFSET + other_god_idx) as FeatureType;
    _add_worker_features(
        board,
        board.workers[other_workers] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        OPPO_WORKER_OFFSET as FeatureType,
        29,
    );

    res
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{board::FullGameState, random_utils::RandomSingleGameStateGenerator};

    #[test]
    fn test_incremental_updates() {
        let game_iter = RandomSingleGameStateGenerator::default();
        let mut acc = LabeledAccumulator::new_from_scratch(&game_iter.peek_unsafe().board);

        for state in game_iter {
            state.print_to_console();

            let from_scratch = LabeledAccumulator::new_from_scratch(&state.board);
            acc.replace_features(build_feature_array(&state.board));

            assert_eq!(from_scratch, acc);
            assert_eq!(from_scratch.evaluate(), acc.evaluate());
        }
    }

    #[test]
    fn test_consistent_choices() {
        // Not really a test I just want to see some example outputs that aren't random
        let mut game_state = FullGameState::new_basic_state_mortals();

        while game_state.board.get_winner().is_none() {
            let from_scratch = LabeledAccumulator::new_from_scratch(&game_state.board);
            let eval = from_scratch.evaluate();

            println!("{:?}: {}", game_state, eval);

            game_state = game_state.get_next_states()[0].clone();
        }
    }
}
