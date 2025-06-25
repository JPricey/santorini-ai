use std::mem;

use crate::board::BoardState;
use crate::board::Player;

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

// const FEATURES: usize = 75 + 2 * 5 * 25;
// const HIDDEN_SIZE: usize = 512;
// static MODEL: Network = unsafe {
//     mem::transmute(*include_bytes!(
//         "../.././models/feat125hidden512-100/quantised.bin"
//     ))
// };

const FEATURES: usize = 375;
const HIDDEN_SIZE: usize = 512;
static MODEL: Network = unsafe {
    mem::transmute(*include_bytes!(
        "../.././models/per_square_h512_wdl75-100/quantised.bin"
    ))
};

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

fn crelu(x: i16) -> i32 {
    let v = i32::from(x.clamp(0, QA as i16));
    v * v
}

impl Network {
    pub fn evaluate(&self, us: &Accumulator) -> i32 {
        let mut output: i32 = QA * MODEL.output_bias as i32;

        for (&input, &weight) in us.vals.iter().zip(&self.output_weights[..HIDDEN_SIZE]) {
            output += crelu(input) * i32::from(weight);
        }
        output *= SCALE;
        output /= i32::from(QA) * i32::from(QA) * i32::from(QB);

        output
    }
}

pub fn _trigger_features_225(acc: &mut Accumulator, board: &BoardState) {
    let mut remaining_spaces: u32 = 0b11111111111111111111;
    for height in (0..4).rev() {
        let mut height_mask = board.height_map[height] & remaining_spaces;
        remaining_spaces ^= height_mask;

        while height_mask > 0 {
            let pos = height_mask.trailing_zeros() as usize;
            height_mask &= height_mask - 1;
            let feature = (pos * 5 + height as usize + 1) as usize;
            acc.add_feature(feature);
        }
    }

    while remaining_spaces > 0 {
        let pos = remaining_spaces.trailing_zeros();
        remaining_spaces &= remaining_spaces - 1;
        let feature = (pos * 5) as usize;
        acc.add_feature(feature);
    }

    fn _add_worker_features(
        board: &BoardState,
        acc: &mut Accumulator,
        mut worker_map: u32,
        feature_offset: usize,
    ) {
        while worker_map > 0 {
            let pos = worker_map.trailing_zeros();
            let worker_height = board.get_height_for_worker(1 << pos);
            worker_map &= worker_map - 1;
            let feature = feature_offset + 5 * pos as usize + worker_height as usize;
            acc.add_feature(feature);
        }
    }
    let (own_workers, other_workers) = match board.current_player {
        Player::One => (0, 1),
        Player::Two => (1, 0),
    };

    _add_worker_features(board, acc, board.workers[own_workers], 75);
    _add_worker_features(board, acc, board.workers[other_workers], 75 + 5 * 25);
}

pub fn _trigger_features_375(acc: &mut Accumulator, board: &BoardState) {
    let mut remaining_spaces: u32 = 0b11111111111111111111;
    let (own_workers, other_workers) = match board.current_player {
        Player::One => (board.workers[0], board.workers[1]),
        Player::Two => (board.workers[1], board.workers[0]),
    };
    for height in (0..4).rev() {
        let mut height_mask = board.height_map[height] & remaining_spaces;
        remaining_spaces ^= height_mask;

        while height_mask > 0 {
            let pos = height_mask.trailing_zeros() as usize;
            height_mask &= height_mask - 1;

            let mut feature = pos * 15 + height + 1;
            let pos_mask = 1 << pos;
            if own_workers & pos_mask > 0 {
                feature += 5;
            } else if other_workers & pos_mask > 0 {
                feature += 10;
            }
            acc.add_feature(feature);
        }
    }

    while remaining_spaces > 0 {
        let pos = remaining_spaces.trailing_zeros() as usize;
        remaining_spaces &= remaining_spaces - 1;

        let mut feature = pos * 15;
        let pos_mask = 1 << pos;
        if own_workers & pos_mask > 0 {
            feature += 5;
        } else if other_workers & pos_mask > 0 {
            feature += 10;
        }
        acc.add_feature(feature);
    }
}

pub fn _trigger_features_double_tuple(acc: &mut Accumulator, board: &BoardState) {
    const HEIGHT_MAP_PER_SQUARE: usize = 5 * 5 * 5 * 5;
    const WORKER_MAP_PER_SQUARE: usize = 3 * 3 * 3 * 3;
    const FEATURES_PER_SQUARE: usize = HEIGHT_MAP_PER_SQUARE + WORKER_MAP_PER_SQUARE;

    const WORKER_POWERS: [usize; 4] = [1, 3, 9, 27];
    const WORKER_MASK: u32 = 0b1100011;

    let mut cur_heights: [u8; 5] = Default::default();
    let mut next_heights: [u8; 5] = Default::default();
    fn _cmp_next_height(board: &BoardState, row_heights: &mut [u8], row: usize) {
        for col in 0..5 {
            let pos = 5 * row + col;
            row_heights[col] = board.get_true_height(1 << pos) as u8;
        }
    }
    _cmp_next_height(board, &mut next_heights, 0);

    let (stm_workers, nstm_workers) = match board.current_player {
        Player::One => (board.workers[0], board.workers[1]),
        Player::Two => (board.workers[1], board.workers[0]),
    };

    for row in 0..4 {
        std::mem::swap(&mut cur_heights, &mut next_heights);
        _cmp_next_height(board, &mut next_heights, row + 1);

        for col in 0..4 {
            let tl = 5 * row + col;
            let square_offset = (4 * row + col) * FEATURES_PER_SQUARE;
            let height_delta = cur_heights[col]
                + 5 * cur_heights[col + 1]
                + 25 * next_heights[col]
                + 125 * next_heights[col + 1];
            acc.add_feature(square_offset + height_delta as usize);
            let mut worker_delta = 0;

            let mut my_workers = (stm_workers >> tl) & WORKER_MASK;
            while my_workers > 0 {
                let mut pos = my_workers.trailing_zeros();
                my_workers &= my_workers - 1;
                if pos > 4 {
                    pos -= 3;
                }
                worker_delta += WORKER_POWERS[pos as usize];
            }

            let mut opp_workers = (nstm_workers >> tl) & WORKER_MASK;
            while opp_workers > 0 {
                let mut pos = opp_workers.trailing_zeros();
                opp_workers &= opp_workers - 1;
                if pos > 4 {
                    pos -= 3;
                }
                worker_delta += 2 * WORKER_POWERS[pos as usize];
            }
            acc.add_feature(square_offset + HEIGHT_MAP_PER_SQUARE + worker_delta);
        }
    }
}

pub fn evaluate(board: &BoardState) -> i32 {
    let mut acc = Accumulator::new();

    // _trigger_features_double_tuple(&mut acc, board);
    // _trigger_features_225(&mut acc, board);
    _trigger_features_375(&mut acc, board);

    let model_eval = MODEL.evaluate(&acc);
    // Scale down if eval is huge
    // if model_eval > 500 {
    //     (500 + (model_eval - 500) / 10).min(600)
    // } else {
    //     -(500 + (-model_eval - 500) / 10).min(600)
    // }
    model_eval
}
