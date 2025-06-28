use std::{arch::x86_64::_mm_hadd_epi16, collections::HashSet, fmt::Debug, mem};

use crate::{
    bitboard::BitBoard, board::BoardState, player::Player,
    random_utils::RandomSingleGameStateGenerator, search::Hueristic,
};

const QA: i32 = 255;
const QB: i32 = 64;

const SCALE: i32 = 400;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LabeledAccumulator {
    board: BoardState,
    pf1: PlayerFeatures,
    pf2: PlayerFeatures,
    p1_acc: Accumulator,
    p2_acc: Accumulator,
}

impl LabeledAccumulator {
    pub fn new_from_scratch(board: &BoardState) -> Self {
        let mut p1_acc = Accumulator::new();
        _extract_board_features(board, |feature| {
            p1_acc.add_feature(feature);
        });

        let mut p2_acc = p1_acc.clone();
        let pf1 = _extract_player_features(board, Player::One);
        let pf2 = _extract_player_features(board, Player::Two);

        for feature in pf1 {
            p1_acc.add_feature(feature as usize + FEATURE_SEGMENT_SIZE);
            p2_acc.add_feature(feature as usize + FEATURE_SEGMENT_SIZE * 2);
        }

        for feature in pf2 {
            p1_acc.add_feature(feature as usize + FEATURE_SEGMENT_SIZE * 2);
            p2_acc.add_feature(feature as usize + FEATURE_SEGMENT_SIZE);
        }

        LabeledAccumulator {
            board: board.clone(),
            p1_acc,
            p2_acc,
            pf1,
            pf2,
        }
    }

    pub fn replace_from_board(&mut self, board: BoardState) {
        fn _handle_height_diff(
            acc: &mut LabeledAccumulator,
            old: BitBoard,
            new: BitBoard,
            height: usize,
        ) -> usize {
            let mut diffs = 0;
            for pos in new & !old {
                let feature: usize = (pos as usize) * 5 + (height as usize);
                acc.p1_acc.add_feature(feature);
                acc.p2_acc.add_feature(feature);
                diffs += 1;
            }

            for pos in old & !new {
                let feature: usize = (pos as usize) * 5 + (height as usize);
                acc.p1_acc.remove_feature(feature);
                acc.p2_acc.remove_feature(feature);
                diffs += 1;
            }

            diffs
        }

        let mut diffs = 0;
        diffs += _handle_height_diff(self, self.board.height_map[3], board.height_map[3], 4);
        diffs += _handle_height_diff(
            self,
            self.board.height_map[2] & !self.board.height_map[3],
            board.height_map[2] & !board.height_map[3],
            3,
        );
        diffs += _handle_height_diff(
            self,
            self.board.height_map[1] & !self.board.height_map[2],
            board.height_map[1] & !board.height_map[2],
            2,
        );
        diffs += _handle_height_diff(
            self,
            self.board.height_map[0] & !self.board.height_map[1],
            board.height_map[0] & !board.height_map[1],
            1,
        );
        diffs += _handle_height_diff(self, !self.board.height_map[0], !board.height_map[0], 0);

        let pf1 = _extract_player_features(&board, Player::One);
        let pf2 = _extract_player_features(&board, Player::Two);

        for p in &pf1 {
            if !self.pf1.contains(p) {
                diffs += 1;
                self.p1_acc.add_feature(*p as usize + FEATURE_SEGMENT_SIZE);
                self.p2_acc
                    .add_feature(*p as usize + 2 * FEATURE_SEGMENT_SIZE);
            }
        }

        for p in &self.pf1 {
            if !pf1.contains(p) {
                diffs += 1;
                self.p1_acc
                    .remove_feature(*p as usize + FEATURE_SEGMENT_SIZE);
                self.p2_acc
                    .remove_feature(*p as usize + 2 * FEATURE_SEGMENT_SIZE);
            }
        }

        for p in &pf2 {
            if !self.pf2.contains(p) {
                diffs += 1;
                self.p1_acc
                    .add_feature(*p as usize + 2 * FEATURE_SEGMENT_SIZE);
                self.p2_acc.add_feature(*p as usize + FEATURE_SEGMENT_SIZE);
            }
        }

        for p in &self.pf2 {
            if !pf2.contains(p) {
                diffs += 1;
                self.p1_acc
                    .remove_feature(*p as usize + 2 * FEATURE_SEGMENT_SIZE);
                self.p2_acc
                    .remove_feature(*p as usize + FEATURE_SEGMENT_SIZE);
            }
        }

        self.pf1 = pf1;
        self.pf2 = pf2;
        self.board = board;

        // if diffs > 10 {
        //     println!("diffs {diffs}");
        // }
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

fn _extract_board_features<F: FnMut(usize)>(board: &BoardState, mut f_callback: F) {
    let mut remaining_spaces = BitBoard::MAIN_SECTION_MASK;

    for height in (0..4).rev() {
        let height_mask = board.height_map[height] & remaining_spaces;
        remaining_spaces ^= height_mask;

        for pos in height_mask {
            let feature = (pos as usize) * 5 + (height as usize) + 1;
            f_callback(feature)
        }
    }

    while remaining_spaces.0 > 0 {
        let pos = remaining_spaces.0.trailing_zeros();
        remaining_spaces.0 &= remaining_spaces.0 - 1;
        let feature = pos * 5;
        f_callback(feature as usize)
    }
}

fn _extract_player_features(board: &BoardState, player: Player) -> PlayerFeatures {
    let mut result = PlayerFeatures::default();
    let mut index = 0;
    let worker_map = board.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;

    for pos in worker_map {
        let worker_height = board.get_height_for_worker(BitBoard::as_mask(pos)) as SmallFeatureType;
        let feature = 5 * (pos as SmallFeatureType) + worker_height;
        result[index] = feature;
        index += 1;
    }
    assert_eq!(index, 2);

    result
}

// pub fn extract_features(board: &BoardState) -> FeatureSet {
//     FeatureSet {
//         board_features: _extract_board_features(board),
//         p1_features: _extract_player_features(board, Player::One),
//         p2_features: _extract_player_features(board, Player::Two),
//     }
// }

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
            acc.replace_from_board(state.board.clone());

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
