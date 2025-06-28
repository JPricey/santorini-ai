use std::{fmt::Debug, mem};

use crate::{bitboard::BitBoard, board::BoardState, player::Player, search::Hueristic};

const QA: i32 = 255;
const QB: i32 = 64;

const SCALE: i32 = 400;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(C, align(64))]
pub struct Accumulator {
    vals: [i16; HIDDEN_SIZE],
}

#[repr(C)]
pub struct Network {
    feature_weights: [Accumulator; FEATURES],
    feature_bias: Accumulator,
    output_weights: [i16; HIDDEN_SIZE],
    output_bias: i16,
}

// const FEATURES: usize = 16 * (5 * 5 * 5 * 5 + 3 * 3 * 3 * 3);
// const HIDDEN_SIZE: usize = 256;
// static MODEL: Network = unsafe {
//     mem::transmute(*include_bytes!(
//         "../.././models/double_tuple-100/quantised.bin"
//     ))
// };

const FEATURES: usize = 375;
const HIDDEN_SIZE: usize = 512;
const FEATURE_COUNT: usize = 29;

type FeatureType = u16;
type FeatureArray = [u16; FEATURE_COUNT];

static MODEL: Network = unsafe {
    mem::transmute(*include_bytes!(
        "../.././models/basic_screlu_fixed-100/quantised.bin"
    ))
};

// const FEATURES: usize = 375;
// const HIDDEN_SIZE: usize = 512;
// static MODEL: Network = unsafe {
//     mem::transmute(*include_bytes!(
//         "../.././models/per_square_fixed-10/quantised.bin"
//     ))
// };

impl Accumulator {
    pub fn new() -> Self {
        MODEL.feature_bias
    }

    pub fn add_feature(&mut self, feature_idx: usize) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&MODEL.feature_weights[feature_idx].vals)
        {
            *i += *d
        }
    }

    pub fn remove_feature(&mut self, feature_idx: usize) {
        for (i, d) in self
            .vals
            .iter_mut()
            .zip(&MODEL.feature_weights[feature_idx].vals)
        {
            *i -= *d
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
    pub fn new_from_scratch(board: &BoardState) -> Self {
        let feature_array = build_feature_array(board);
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
                self.accumulator.remove_feature(*current as usize);
                self.accumulator.add_feature(new as usize);
                *current = new;
            }
        }
    }

    pub fn replace_from_board(&mut self, board: &BoardState) {
        self.replace_features(build_feature_array(board))
    }

    pub fn evaluate(&self) -> Hueristic {
        MODEL.evaluate(&self.accumulator)
    }
}

#[allow(dead_code)]
fn screlu(x: i16) -> i32 {
    let v = i32::from(x.clamp(0, QA as i16));
    v * v
}

#[allow(dead_code)]
fn crelu(x: i16) -> i32 {
    let v = i32::from(x.clamp(0, QA as i16));
    v
}

impl Network {
    #[inline(never)]
    pub fn evaluate(&self, us: &Accumulator) -> i32 {
        // Crelu init:
        // let mut output: i32 = MODEL.output_bias as i32;
        // Screlu init:
        let mut output: i32 = 0;

        for (&input, &weight) in us.vals.iter().zip(&self.output_weights[..HIDDEN_SIZE]) {
            output += screlu(input) * i32::from(weight);
        }
        // Screlu only adjustment
        output = (output / QA) + MODEL.output_bias as i32;

        output *= SCALE;
        output /= i32::from(QA) * i32::from(QB);

        output
    }
}

pub fn build_feature_array(board: &BoardState) -> FeatureArray {
    let mut res = FeatureArray::default();
    for pos in 0..25 {
        res[pos] = (pos * 5) as FeatureType
            + board.get_true_height(BitBoard::as_mask_u8(pos as u8)) as FeatureType;
    }

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
                feature_offset + 5 * (pos as FeatureType) + worker_height as FeatureType;
            features[index] = feature;
            index += 1
        }
        index
    }
    let (own_workers, other_workers) = match board.current_player {
        Player::One => (0, 1),
        Player::Two => (1, 0),
    };

    _add_worker_features(
        board,
        board.workers[own_workers] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        5 * 25,
        25,
    );
    _add_worker_features(
        board,
        board.workers[other_workers] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        5 * 25 * 2,
        27,
    );

    res
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::random_utils::RandomSingleGameStateGenerator;

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
}
