use std::mem;

use crate::{bitboard::BitBoard, board::BoardState, player::Player, search::Hueristic};

const QA: i32 = 255;
const QB: i32 = 64;

const SCALE: i32 = 400;

#[derive(Clone, Copy)]
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

#[derive(Clone)]
struct LabeledAccumulator {
    feature_array: FeatureArray,
    accumulator: Accumulator,
}

impl LabeledAccumulator {
    pub fn new_from_scratch(board: &BoardState) -> Self {
        let mut feature_array = get_feature_array(board);
        let mut accumulator = Accumulator::new();

        for feature in feature_array {
            accumulator.add_feature(feature);
        }

        LabeledAccumulator {
            feature_array,
            accumulator,
        }
    }

    pub fn replace_features(&mut self, feature_array: FeatureArray) {
        for (current, new) in self.feature_array.iter_mut().zip(feature_array.iter) {
            if current != new {
                self.accumulator.remove_feature(current);
                self.accumulator.add(new);
                *current = new;
            }
        }
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

pub fn get_feature_array(board: &BoardState) -> FeatureArray {
    let mut res = FeatureArray::default();
    let mut index = 0;

    let mut remaining_spaces = BitBoard((1 << 25) - 1);
    for height in (0..4).rev() {
        let mut height_mask = board.height_map[height] & remaining_spaces;
        remaining_spaces ^= height_mask;

        while height_mask.0 > 0 {
            let pos = height_mask.0.trailing_zeros() as FeatureType;
            height_mask.0 &= height_mask.0 - 1;
            let feature = (pos * 5(height as FeatureType) + 1) as FeatureType;
            res[index] = feature;
            index += 1;
        }
    }

    while remaining_spaces.0 > 0 {
        let pos = remaining_spaces.0.trailing_zeros();
        remaining_spaces.0 &= remaining_spaces.0 - 1;
        let feature = (pos * 5) as FeatureType;
        res[index] = feature;
        index += 1;
    }

    assert_eq!(index, 25);
    res[0..index].sort();

    fn _add_worker_features(
        mut worker_map: u32,
        features: &mut FeatureArray,
        index: usize,
    ) -> usize {
        while worker_map > 0 {
            let pos = worker_map.trailing_zeros();
            let worker_height = board.get_height_for_worker(BitBoard(1 << pos));
            worker_map &= worker_map - 1;
            let feature = feature_offset + 5 * pos as usize + worker_height as usize;
            features[index] = feature;
            index += 1
        }
        index
    }
    let (own_workers, other_workers) = match board.current_player {
        Player::One => (0, 1),
        Player::Two => (1, 0),
    };

    index = _add_worker_features(board.workers[own_workers].0, &mut features, index);
    index = _add_worker_features(board.workers[other_workers].0, &mut features, index);

    res
}
