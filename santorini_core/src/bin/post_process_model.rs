use std::mem;

use santorini_core::nnue::{MATCHUP_BIT_FEATURE_OFFSET, NNUE_GOD_COUNT};

const HIDDEN_SIZE: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C, align(64))]
pub struct Align64<T>(pub T);

#[derive(Clone, PartialEq, Eq, Debug, Copy)]
struct Accumulator {
    pub vals: Align64<[i16; HIDDEN_SIZE]>,
}

const TOTAL_NUM_FEATURES_WITH_EXTRA_BITS: usize = 1364;
const NORMALIZING_FEATURES_START: usize = 1306;

#[repr(C)]
#[derive(Clone, Debug)]
struct Network<const FEATURES: usize> {
    pub feature_weights: [Accumulator; FEATURES],
    pub feature_bias: Accumulator,
    pub output_weights: [i16; HIDDEN_SIZE],
    pub output_bias: i16,
}

impl Default for Network<{ NORMALIZING_FEATURES_START }> {
    fn default() -> Self {
        Self {
            feature_weights: [Accumulator {
                vals: Align64([0; HIDDEN_SIZE]),
            }; NORMALIZING_FEATURES_START],
            feature_bias: Accumulator {
                vals: Align64([0; HIDDEN_SIZE]),
            },
            output_weights: [0; HIDDEN_SIZE],
            output_bias: 0,
        }
    }
}

static FULL_MODEL: Network<TOTAL_NUM_FEATURES_WITH_EXTRA_BITS> =
    unsafe { mem::transmute(*include_bytes!("../../.././models/matchup_bit.bin")) };

fn main() {
    eprintln!("NORMALIZING_FEATURES_START: {}", NORMALIZING_FEATURES_START);
    eprintln!("total: {}", NORMALIZING_FEATURES_START + 2 * NNUE_GOD_COUNT);

    let mut model_clone = FULL_MODEL.clone();

    for god1 in 0..NNUE_GOD_COUNT {
        let g1_idx = NORMALIZING_FEATURES_START + god1;

        for god2 in 0..NNUE_GOD_COUNT {
            let g2_idx = NORMALIZING_FEATURES_START + NNUE_GOD_COUNT + god2;
            let matchup_feature_delta = (god1 * NNUE_GOD_COUNT) + god2;
            let matchup_feature_final = matchup_feature_delta + MATCHUP_BIT_FEATURE_OFFSET as usize;

            for i in 0..HIDDEN_SIZE {
                model_clone.feature_weights[matchup_feature_final].vals.0[i] +=
                    model_clone.feature_weights[g1_idx].vals.0[i]
                        + model_clone.feature_weights[g2_idx].vals.0[i];
            }
        }
    }

    let mut smaller_model: Network<NORMALIZING_FEATURES_START> = Default::default();
    smaller_model
        .feature_weights
        .copy_from_slice(&model_clone.feature_weights[0..NORMALIZING_FEATURES_START]);
    smaller_model.feature_bias = model_clone.feature_bias;
    smaller_model.output_weights = model_clone.output_weights;
    smaller_model.output_bias = model_clone.output_bias;

    // eprintln!("{:?}", smaller_model);

    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            &smaller_model as *const Network<NORMALIZING_FEATURES_START> as *const u8,
            std::mem::size_of::<Network<NORMALIZING_FEATURES_START>>(),
        )
    };
    std::fs::write("matchup_bit_post_processed.bin", bytes).unwrap();
}
