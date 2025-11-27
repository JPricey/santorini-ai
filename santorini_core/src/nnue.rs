use std::{
    fmt::Debug,
    mem,
    ops::{Deref, DerefMut},
    simd::{Simd, cmp::SimdOrd, num::SimdInt},
};

use arrayvec::ArrayVec;

use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodData},
    gods::{GOD_FEATURE_OFFSETS, GodName, TOTAL_GOD_DATA_FEATURE_COUNT_FOR_NNUE},
    player::Player,
    search::Heuristic,
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
    pub feature_weights: [Accumulator; TOTAL_NUM_FEATURES],
    pub feature_bias: Accumulator,
    pub output_weights: [i16; HIDDEN_SIZE],
    pub output_bias: i16,
    pub asdf: u32,
}

type FType = u16;
// pub const NNUE_GOD_COUNT: usize = 29;
pub const NNUE_GOD_COUNT: usize = 39;

// Neutral Features
const BOARD_FEATURES_COUNT: usize = 5 * 25;
const MATCHUP_BIT_FEATURES_COUNT: usize = NNUE_GOD_COUNT * NNUE_GOD_COUNT;

const BOARD_FEATURES_OFFSET: FType = 0;
pub const MATCHUP_BIT_FEATURE_OFFSET: FType = BOARD_FEATURES_COUNT as FType;

const TOTAL_BASE_FEATURES_COUNT: usize = BOARD_FEATURES_COUNT + MATCHUP_BIT_FEATURES_COUNT;

// Per-side main features
const PER_GOD_MAIN_FEATURE_SECTION_START: usize = TOTAL_BASE_FEATURES_COUNT;

const WORKER_FEATURES_COUNT: usize = 25 * 4;

const ACTIVE_PLAYER_OFFSET: FType = PER_GOD_MAIN_FEATURE_SECTION_START as FType;
const OPPO_PLAYER_OFFSET: FType = ACTIVE_PLAYER_OFFSET + PER_SIDE_MAIN_FEATURES_COUNT as FType;

const PLAYER_WORKERS_OFFSET: FType = 0;
const PLAYER_DATAS_OFFSET: FType = PLAYER_WORKERS_OFFSET + WORKER_FEATURES_COUNT as FType;

const PER_SIDE_MAIN_FEATURES_COUNT: usize =
    WORKER_FEATURES_COUNT + TOTAL_GOD_DATA_FEATURE_COUNT_FOR_NNUE;
const PER_GOD_MAIN_FEATURE_SECTION_TOTAL_SIZE: usize = PER_SIDE_MAIN_FEATURES_COUNT * 2;

// Per-side  features normalizing features
pub const PER_GOD_NORMALIZING_FEATURE_SECTION_START: usize =
    PER_GOD_MAIN_FEATURE_SECTION_START + PER_GOD_MAIN_FEATURE_SECTION_TOTAL_SIZE;

#[allow(dead_code)]
const ACTIVE_PLAYER_GOD_OFFSET: FType = PER_GOD_NORMALIZING_FEATURE_SECTION_START as FType;
#[allow(dead_code)]
const OPPO_PLAYER_GOD_OFFSET: FType = ACTIVE_PLAYER_GOD_OFFSET + NNUE_GOD_COUNT as FType;

#[allow(dead_code)]
const PER_GOD_NORMALIZING_FEATURE_TOTAL_SIZE: usize = NNUE_GOD_COUNT * 2;

pub const TOTAL_NUM_FEATURES: usize = TOTAL_BASE_FEATURES_COUNT
    + PER_GOD_MAIN_FEATURE_SECTION_TOTAL_SIZE
    // + PER_GOD_NORMALIZING_FEATURE_TOTAL_SIZE
    ;
pub const HIDDEN_SIZE: usize = 1024;

const MAX_DYNAMIC_FEATURE_COUNT: usize = 10 * 2; // Workers, extra datas

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FeatureSet {
    // ordered_features are always present, and will always map 1-1 between different board states
    // Ex: the height of A3 will always be at the 3rd index, regardless of feature values
    pub ordered_features: [u16; 26], // Heights + matchup
    // pub ordered_features: [u16; 28], // Heights + matchup
    pub dynamic_features: ArrayVec<u16, MAX_DYNAMIC_FEATURE_COUNT>,
}

// pub static MODEL: Network = unsafe { mem::transmute(*include_bytes!("../.././models/full.bin")) };
pub static MODEL: Network = unsafe { mem::transmute(*include_bytes!("../.././models/final.bin")) };

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

#[derive(Clone)]
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
        for feature in &self.feature_set.dynamic_features {
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

        let mut adds: ArrayVec<FType, MAX_DYNAMIC_FEATURE_COUNT> = Default::default();
        let mut subs: ArrayVec<FType, MAX_DYNAMIC_FEATURE_COUNT> = Default::default();

        let mut cur_i = 0;
        let mut other_i = 0;

        loop {
            if cur_i >= self.feature_set.dynamic_features.len() {
                while other_i < feature_set.dynamic_features.len() {
                    adds.push(feature_set.dynamic_features[other_i]);
                    other_i += 1;
                }
                break;
            } else if other_i >= feature_set.dynamic_features.len() {
                while cur_i < self.feature_set.dynamic_features.len() {
                    subs.push(self.feature_set.dynamic_features[cur_i]);
                    cur_i += 1;
                }
                break;
            }

            if self.feature_set.dynamic_features[cur_i] == feature_set.dynamic_features[other_i] {
                cur_i += 1;
                other_i += 1;
            } else if self.feature_set.dynamic_features[cur_i]
                > feature_set.dynamic_features[other_i]
            {
                adds.push(feature_set.dynamic_features[other_i]);
                other_i += 1;
            } else {
                subs.push(self.feature_set.dynamic_features[cur_i]);
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

        self.feature_set.dynamic_features = feature_set.dynamic_features;
    }

    pub fn replace_from_state(&mut self, state: &FullGameState) {
        self.replace_features(build_feature_set(
            &state.board,
            state.gods[0].model_god_name,
            state.gods[1].model_god_name,
        ))
    }

    pub fn replace_from_board(&mut self, board: &BoardState, god1: GodName, god2: GodName) {
        self.replace_features(build_feature_set(board, god1, god2))
    }

    pub fn evaluate(&self) -> Heuristic {
        MODEL.evaluate(&self.accumulator)
    }
}

impl Network {
    pub fn evaluate(&self, us: &Accumulator) -> Heuristic {
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

        output *= SCALE;
        output /= i32::from(QA) * i32::from(QB);

        output as Heuristic
    }
}

pub const NNUE_MORPHEUS_MAX_BLOCKS_INCLUSIVE: u32 = 10;
pub fn emit_god_data_features<Extractor: FnMut(FType)>(
    god: GodName,
    data: GodData,
    mut f: Extractor,
) {
    match god {
        GodName::Athena => {
            if data > 0 {
                f(0)
            }
        }
        GodName::Morpheus => {
            if data > 0 {
                f((data as FType).min(NNUE_MORPHEUS_MAX_BLOCKS_INCLUSIVE as FType) - 1)
            }
        }
        GodName::Aeolus => {
            if data > 0 {
                f(data as FType - 1)
            }
        }
        GodName::Europa | GodName::Hippolyta | GodName::Selene => {
            if data > 0 {
                f(data.trailing_zeros() as FType)
            }
        }
        GodName::Clio => {
            let remaining_coins = 3 - (data >> 25);
            let coins = BitBoard(data & BitBoard::MAIN_SECTION_MASK.0);
            for c in coins {
                f(c as FType)
            }

            if remaining_coins > 0 {
                f(25);
            }
        }
        _ => (),
    }
}

const fn _matchup_feature(god1: usize, god2: usize) -> FType {
    MATCHUP_BIT_FEATURE_OFFSET + (god1 as FType) * (NNUE_GOD_COUNT as FType) + (god2 as FType)
}

pub fn build_feature_set(board: &BoardState, god1: GodName, god2: GodName) -> FeatureSet {
    let mut res = FeatureSet::default();
    for pos in 0..25 {
        res.ordered_features[pos] =
            BOARD_FEATURES_OFFSET + (pos * 5) as FType + board.get_height(pos.into()) as FType;
    }
    let (own_god, other_god) = match board.current_player {
        Player::One => (god1, god2),
        Player::Two => (god2, god1),
    };
    let own_god_idx = own_god as usize;
    let other_god_idx = other_god as usize;

    res.ordered_features[25] = _matchup_feature(own_god_idx, other_god_idx);

    // res.ordered_features[26] = ACTIVE_PLAYER_GOD_OFFSET + own_god_idx as FType;
    // res.ordered_features[27] = OPPO_PLAYER_GOD_OFFSET + other_god_idx as FType;

    fn _add_worker_features(
        board: &BoardState,
        worker_map: BitBoard,
        features: &mut FeatureSet,
        feature_offset: FType,
    ) {
        for pos in worker_map {
            let worker_height = board.get_height(pos);
            let feature: FType = feature_offset + 4 * (pos as FType) + worker_height as FType;
            features.dynamic_features.push(feature);
        }
    }

    fn _add_data_features(
        god: GodName,
        data: GodData,
        features: &mut FeatureSet,
        feature_offset: FType,
    ) {
        emit_god_data_features(god, data, |f| {
            let res = feature_offset + GOD_FEATURE_OFFSETS[god as usize] as FType + f as FType;
            features.dynamic_features.push(res);
        });
    }

    let (own_idx, other_idx) = match board.current_player {
        Player::One => (0, 1),
        Player::Two => (1, 0),
    };

    _add_worker_features(
        board,
        board.workers[own_idx] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        ACTIVE_PLAYER_OFFSET + PLAYER_WORKERS_OFFSET,
    );

    _add_data_features(
        own_god,
        board.god_data[own_idx],
        &mut res,
        ACTIVE_PLAYER_OFFSET + PLAYER_DATAS_OFFSET,
    );

    _add_worker_features(
        board,
        board.workers[other_idx] & BitBoard::MAIN_SECTION_MASK,
        &mut res,
        OPPO_PLAYER_OFFSET + PLAYER_WORKERS_OFFSET,
    );

    _add_data_features(
        other_god,
        board.god_data[other_idx],
        &mut res,
        OPPO_PLAYER_OFFSET + PLAYER_DATAS_OFFSET,
    );

    res
}

#[cfg(test)]
mod tests {
    use crate::{gods::ALL_GODS_BY_ID, nnue::NNUE_GOD_COUNT};

    #[test]
    fn test_nnue_id_is_valid() {
        for god_power in ALL_GODS_BY_ID.iter() {
            assert!(
                (god_power.model_god_name as usize) < NNUE_GOD_COUNT,
                "God {:?} has an invalid nnue id {:?}",
                god_power.god_name,
                god_power.model_god_name
            );
        }
    }

    #[test]
    fn test_nnue_id_should_be_set() {
        for god_power in ALL_GODS_BY_ID.iter() {
            if (god_power.god_name as usize) < NNUE_GOD_COUNT {
                assert_eq!(
                    god_power.god_name, god_power.model_god_name,
                    "God {:?} should use their own NNUE model",
                    god_power.god_name,
                );
            }
        }
    }

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
