use std::{
    fmt::Debug,
    mem,
    ops::{Deref, DerefMut},
    simd::{Simd, cmp::SimdOrd, num::SimdInt},
};

use arrayvec::ArrayVec;

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
const MAX_WORKER_FEATURE_COUNT: usize = 6;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FeatureSet {
    pub ordered_features: [u16; 27],
    pub worker_features: ArrayVec<u16, 6>,
}

pub static MODEL: Network =
    unsafe { mem::transmute(*include_bytes!("../.././models/gods-labeled-3.bin")) };

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

    pub fn add_add_feature(&mut self, idx1: usize, idx2: usize) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let add1 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[idx1].vals[i..i + FEATURE_LANES],
            );
            let add2 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[idx2].vals[i..i + FEATURE_LANES],
            );
            let sum = acc + add1 + add2;
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

    pub fn sub_sub_feature(&mut self, idx1: usize, idx2: usize) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let sub1 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[idx1].vals[i..i + FEATURE_LANES],
            );
            let sub2 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[idx2].vals[i..i + FEATURE_LANES],
            );
            let sum = acc - sub1 - sub2;
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

    pub fn add_add_remove_feature(&mut self, add_idx: usize, add_idx_2: usize, sub_idx: usize) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let add = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[add_idx].vals[i..i + FEATURE_LANES],
            );
            let add2 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[add_idx_2].vals[i..i + FEATURE_LANES],
            );
            let sub = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[sub_idx].vals[i..i + FEATURE_LANES],
            );
            let sum = add + add2 - sub + acc;
            sum.copy_to_slice(&mut self.vals.0[i..i + FEATURE_LANES]);
        }
    }

    pub fn add_rem_rem_feature(&mut self, add_idx: usize, sub_idx: usize, sub_idx_2: usize) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let add = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[add_idx].vals[i..i + FEATURE_LANES],
            );
            let sub = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[sub_idx].vals[i..i + FEATURE_LANES],
            );
            let sub2 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[sub_idx_2].vals[i..i + FEATURE_LANES],
            );
            let sum = add - sub - sub2 + acc;
            sum.copy_to_slice(&mut self.vals.0[i..i + FEATURE_LANES]);
        }
    }

    pub fn add_add_rem_rem_feature(
        &mut self,
        add_idx: usize,
        add_idx_2: usize,
        sub_idx: usize,
        sub_idx_2: usize,
    ) {
        for i in (0..HIDDEN_SIZE).step_by(FEATURE_LANES) {
            let acc = Simd::<i16, FEATURE_LANES>::from_slice(&self.vals.0[i..i + FEATURE_LANES]);
            let add = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[add_idx].vals[i..i + FEATURE_LANES],
            );
            let add2 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[add_idx_2].vals[i..i + FEATURE_LANES],
            );
            let sub = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[sub_idx].vals[i..i + FEATURE_LANES],
            );
            let sub2 = Simd::<i16, FEATURE_LANES>::from_slice(
                &MODEL.feature_weights[sub_idx_2].vals[i..i + FEATURE_LANES],
            );
            let sum = add + add2 - sub - sub2 + acc;
            sum.copy_to_slice(&mut self.vals.0[i..i + FEATURE_LANES]);
        }
    }
}

// TODO: equality should be for features only
#[derive(Clone, PartialEq, Eq)]
pub struct LabeledAccumulator {
    feature_set: FeatureSet,
    accumulator: Accumulator,
}

impl Debug for LabeledAccumulator {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("LabeledAccumulator")
            .field("feature_array", &self.feature_set)
            .finish()
    }
}

impl LabeledAccumulator {
    pub fn new_from_scratch(board: &BoardState, god1: GodName, god2: GodName) -> Self {
        let feature_set = build_feature_set(board, god1, god2);

        let mut res = LabeledAccumulator {
            feature_set,
            accumulator: Accumulator::new(),
        };

        res._apply_own_feature_set();

        res
    }

    fn _apply_own_feature_set(&mut self) {
        for feature in &self.feature_set.ordered_features {
            self.accumulator.add_feature(*feature as usize);
        }
        for feature in &self.feature_set.worker_features {
            self.accumulator.add_feature(*feature as usize);
        }
    }

    fn replace_features(&mut self, feature_set: FeatureSet) {
        for (current, &new) in self
            .feature_set
            .ordered_features
            .iter_mut()
            .zip(feature_set.ordered_features.iter())
        {
            if *current != new {
                self.accumulator
                    .add_remove_feature(new as usize, *current as usize);
                *current = new;
            }
        }

        let mut adds: ArrayVec<FeatureType, MAX_WORKER_FEATURE_COUNT> = Default::default();
        let mut subs: ArrayVec<FeatureType, MAX_WORKER_FEATURE_COUNT> = Default::default();

        let mut cur_i = 0;
        let mut other_i = 0;

        loop {
            if cur_i >= self.feature_set.worker_features.len() {
                while other_i < feature_set.worker_features.len() {
                    adds.push(feature_set.worker_features[other_i]);
                    other_i += 1;
                }
                break;
            } else if other_i >= feature_set.worker_features.len() {
                while cur_i < self.feature_set.worker_features.len() {
                    subs.push(self.feature_set.worker_features[cur_i]);
                    cur_i += 1;
                }
                break;
            }

            if self.feature_set.worker_features[cur_i] == feature_set.worker_features[other_i] {
                cur_i += 1;
                other_i += 1;
            } else if self.feature_set.worker_features[cur_i] > feature_set.worker_features[other_i]
            {
                adds.push(feature_set.worker_features[other_i]);
                other_i += 1;
            } else {
                subs.push(self.feature_set.worker_features[cur_i]);
                cur_i += 1;
            }
        }

        loop {
            match (adds.pop(), adds.pop(), subs.pop(), subs.pop()) {
                (Some(add1), Some(add2), Some(sub1), Some(sub2)) => {
                    self.accumulator.add_add_rem_rem_feature(
                        add1 as usize,
                        add2 as usize,
                        sub1 as usize,
                        sub2 as usize,
                    );
                }
                (Some(add1), Some(add2), Some(sub1), None) => {
                    self.accumulator.add_add_remove_feature(
                        add1 as usize,
                        add2 as usize,
                        sub1 as usize,
                    );
                }
                (Some(add1), None, Some(sub1), Some(sub2)) => {
                    self.accumulator.add_rem_rem_feature(
                        add1 as usize,
                        sub1 as usize,
                        sub2 as usize,
                    );
                }
                (Some(add1), None, Some(sub1), None) => {
                    self.accumulator
                        .add_remove_feature(add1 as usize, sub1 as usize);
                }
                (Some(add1), Some(add2), None, None) => {
                    self.accumulator
                        .add_add_feature(add1 as usize, add2 as usize);
                }
                (Some(add1), None, None, None) => {
                    self.accumulator.add_feature(add1 as usize);
                    break;
                }
                (None, None, Some(sub1), Some(sub2)) => {
                    self.accumulator
                        .sub_sub_feature(sub1 as usize, sub2 as usize);
                }
                (None, None, Some(sub1), None) => {
                    self.accumulator.remove_feature(sub1 as usize);
                    break;
                }
                (None, None, None, None) => break,
                _ => unreachable!(),
            }
        }

        self.feature_set.worker_features = feature_set.worker_features;
    }

    pub fn replace_from_board(&mut self, board: &BoardState, god1: GodName, god2: GodName) {
        self.replace_features(build_feature_set(board, god1, god2))
    }

    pub fn replace_from_board_with_possible_reset(
        &mut self,
        board: &BoardState,
        god1: GodName,
        god2: GodName,
    ) {
        let new_feature_set = build_feature_set(board, god1, god2);
        let mut diffs = 0;

        for (a, b) in self
            .feature_set
            .ordered_features
            .iter()
            .zip(new_feature_set.ordered_features.iter())
        {
            if a != b {
                diffs += 1;
            }
        }

        if diffs > 15 {
            self.accumulator.vals.fill(0);
            self.feature_set = new_feature_set;
            self._apply_own_feature_set();
        } else {
            self.replace_features(new_feature_set);
        }
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

fn build_feature_set(board: &BoardState, god1: GodName, god2: GodName) -> FeatureSet {
    let mut res = FeatureSet::default();
    for pos in 0..25 {
        res.ordered_features[pos] =
            (pos * 5) as FeatureType + board.get_height(pos.into()) as FeatureType;
    }
    let (own_god_idx, other_god_idx) = match board.current_player {
        Player::One => (god1 as usize, god2 as usize),
        Player::Two => (god2 as usize, god1 as usize),
    };
    res.ordered_features[25] = (ACTIVE_PLAYER_OFFSET + own_god_idx) as FeatureType;
    res.ordered_features[26] = (OPPO_OFFSET + other_god_idx) as FeatureType;

    fn _add_worker_features(
        board: &BoardState,
        worker_map: BitBoard,
        features: &mut FeatureSet,
        feature_offset: FeatureType,
    ) {
        for pos in worker_map {
            let worker_height = board.get_height(pos);
            let feature: FeatureType =
                feature_offset + 4 * (pos as FeatureType) + worker_height as FeatureType;
            features.worker_features.push(feature);
        }
    }
    let (own_workers, other_workers) = match board.current_player {
        Player::One => (0, 1),
        Player::Two => (1, 0),
    };

    _add_worker_features(
        board,
        board.workers[own_workers] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        ACTIVE_PLAYER_WORKER_OFFSET as FeatureType,
    );

    _add_worker_features(
        board,
        board.workers[other_workers] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        OPPO_WORKER_OFFSET as FeatureType,
    );

    res
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{board::FullGameState, random_utils::RandomSingleGameStateGenerator};

    // #[test]
    // fn test_incremental_updates() {
    //     let game_iter = RandomSingleGameStateGenerator::default();
    //     let mut acc = LabeledAccumulator::new_from_scratch(&game_iter.peek_unsafe().board);

    //     for state in game_iter {
    //         state.print_to_console();

    //         let from_scratch = LabeledAccumulator::new_from_scratch(&state.board);
    //         acc.replace_features(build_feature_array(&state.board));

    //         assert_eq!(from_scratch, acc);
    //         assert_eq!(from_scratch.evaluate(), acc.evaluate());
    //     }
    // }

    // #[test]
    // fn test_consistent_choices() {
    //     // Not really a test I just want to see some example outputs that aren't random
    //     let mut game_state = FullGameState::new_basic_state_mortals();

    //     while game_state.board.get_winner().is_none() {
    //         let from_scratch = LabeledAccumulator::new_from_scratch(&game_state.board);
    //         let eval = from_scratch.evaluate();

    //         println!("{:?}: {}", game_state, eval);

    //         game_state = game_state.get_next_states()[0].clone();
    //     }
    // }
}
