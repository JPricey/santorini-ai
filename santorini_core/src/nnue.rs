use std::{fmt::Debug, mem};

use crate::{bitboard::BitBoard, board::BoardState, player::Player, random_utils::RandomSingleGameStateGenerator, search::Hueristic};

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

const FEATURE_SEGMENT_SIZE: usize = 25 * 5;

// type FeatureType = u16;
// type FeatureArray = [u16; FEATURE_COUNT];

type SmallFeatureType = u8;
type BoardFeatures = [SmallFeatureType; 25];
type PlayerFeatures = [SmallFeatureType; 2];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureSet {
    board_features: BoardFeatures,
    p1_features: PlayerFeatures,
    p2_features: PlayerFeatures,
}

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
    features: FeatureSet,
    p1_acc: Accumulator,
    p2_acc: Accumulator,
    board: BoardState,
}

impl Debug for LabeledAccumulator {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("LabeledAccumulator")
            .field("features", &self.features)
            .finish()
    }
}

impl LabeledAccumulator {
    pub fn new_from_scratch(board: &BoardState) -> Self {
        let features = extract_features(board);
        let mut p1_acc = Accumulator::new();

        for feature in features.board_features {
            p1_acc.add_feature(feature as usize);
        }
        let mut p2_acc = p1_acc.clone();

        for feature in features.p1_features {
            let p1_f = FEATURE_SEGMENT_SIZE + feature as usize;
            let p2_f = 2 * FEATURE_SEGMENT_SIZE + feature as usize;

            p1_acc.add_feature(p1_f);
            p2_acc.add_feature(p2_f);
        }

        for feature in features.p2_features {
            let p1_f = 2 * FEATURE_SEGMENT_SIZE + feature as usize;
            let p2_f = FEATURE_SEGMENT_SIZE + feature as usize;

            p1_acc.add_feature(p1_f);
            p2_acc.add_feature(p2_f);
        }

        LabeledAccumulator {
            features,
            p1_acc,
            p2_acc,
            board: board.clone(),
        }
    }

    pub fn replace_features(&mut self, feature_set: FeatureSet) {
        let mut diff_count = 0;
        for (current, &new) in self
            .features
            .board_features
            .iter_mut()
            .zip(feature_set.board_features.iter())
        {
            if *current != new {
                self.p1_acc.remove_feature(*current as usize);
                self.p2_acc.remove_feature(*current as usize);

                self.p1_acc.add_feature(new as usize);
                self.p2_acc.add_feature(new as usize);
                *current = new;
                diff_count += 1;
            }
        }

        for (current, &new) in self
            .features
            .p1_features
            .iter_mut()
            .zip(feature_set.p1_features.iter())
        {
            if *current != new {
                diff_count += 1;
                self.p1_acc
                    .remove_feature(FEATURE_SEGMENT_SIZE + *current as usize);
                self.p2_acc
                    .remove_feature(2 * FEATURE_SEGMENT_SIZE + *current as usize);

                self.p1_acc.add_feature(FEATURE_SEGMENT_SIZE + new as usize);
                self.p2_acc
                    .add_feature(2 * FEATURE_SEGMENT_SIZE + new as usize);
                *current = new;
                diff_count += 1;
            }
        }

        for (current, &new) in self
            .features
            .p2_features
            .iter_mut()
            .zip(feature_set.p2_features.iter())
        {
            if *current != new {
                diff_count += 1;
                self.p1_acc
                    .remove_feature(2 * FEATURE_SEGMENT_SIZE + *current as usize);
                self.p2_acc
                    .remove_feature(FEATURE_SEGMENT_SIZE + *current as usize);

                self.p1_acc
                    .add_feature(2 * FEATURE_SEGMENT_SIZE + new as usize);
                self.p2_acc.add_feature(FEATURE_SEGMENT_SIZE + new as usize);
                *current = new;
                diff_count += 1;
            }
        }

        if diff_count > 5 {
            dbg!(diff_count);
        }
    }

    pub fn replace_from_board(&mut self, board: &BoardState) {
        self.replace_features(extract_features(board))
    }

    pub fn evaluate(&self, player: Player) -> Hueristic {
        match player {
            Player::One => MODEL.evaluate(&self.p1_acc),
            Player::Two => MODEL.evaluate(&self.p2_acc),
        }
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

fn _extract_board_features(board: &BoardState) -> BoardFeatures {
    let mut res = BoardFeatures::default();
    let mut index = 0;

    let mut remaining_spaces = BitBoard((1 << 25) - 1);
    for height in (0..4).rev() {
        let mut height_mask = board.height_map[height] & remaining_spaces;
        remaining_spaces ^= height_mask;

        while height_mask.0 > 0 {
            let pos = height_mask.0.trailing_zeros() as SmallFeatureType;
            height_mask.0 &= height_mask.0 - 1;
            let feature = pos * 5 + (height as SmallFeatureType) + 1;
            res[index] = feature;
            index += 1;
        }
    }

    while remaining_spaces.0 > 0 {
        let pos = remaining_spaces.0.trailing_zeros() as SmallFeatureType;
        remaining_spaces.0 &= remaining_spaces.0 - 1;
        let feature = pos * 5;
        res[index] = feature;
        index += 1;
    }

    assert_eq!(index, 25);
    res.sort();

    res
}

fn _extract_player_features(board: &BoardState, player: Player) -> PlayerFeatures {
    let worker_map = board.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;

    let mut res = PlayerFeatures::default();
    let mut index = 0;

    for pos in worker_map {
        let worker_height = board.get_height_for_worker(BitBoard::as_mask(pos));
        let feature = 5 * (pos as SmallFeatureType) + (worker_height as SmallFeatureType);
        res[index] = feature;
        index += 1
    }
    assert_eq!(index, 2);

    res
}

pub fn extract_features(board: &BoardState) -> FeatureSet {
    FeatureSet {
        board_features: _extract_board_features(board),
        p1_features: _extract_player_features(board, Player::One),
        p2_features: _extract_player_features(board, Player::Two),
    }
}

#[cfg(test)]
mod tests {
    use std::array::from_mut;

    use super::*;
    use crate::{
        board::BoardState,
        random_utils::{GameStateFuzzer, RandomSingleGameStateGenerator},
    };

    #[test]
    fn test_incremental_updates() {
        let game_iter = RandomSingleGameStateGenerator::default();
        let mut acc = LabeledAccumulator::new_from_scratch(&game_iter.peek_unsafe().board);

        for state in game_iter {
            state.print_to_console();

            let from_scratch = LabeledAccumulator::new_from_scratch(&state.board);
            acc.replace_features(extract_features(&state.board));

            assert_eq!(from_scratch, acc);
            assert_eq!(
                from_scratch.evaluate(Player::One),
                acc.evaluate(Player::One)
            );
            assert_eq!(
                from_scratch.evaluate(Player::Two),
                acc.evaluate(Player::Two)
            );
        }
    }
}
