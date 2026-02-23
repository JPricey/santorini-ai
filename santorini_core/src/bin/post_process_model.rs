use santorini_core::{
    gods::TOTAL_GOD_DATA_FEATURE_COUNT_FOR_NNUE,
    nnue::{MATCHUP_BIT_FEATURE_OFFSET, TOTAL_NUM_FEATURES},
};

const HIDDEN_SIZE: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C, align(64))]
pub struct Align64<T>(pub T);

#[derive(Clone, PartialEq, Eq, Debug, Copy)]
struct Accumulator {
    pub vals: Align64<[i16; HIDDEN_SIZE]>,
}

// Matches santorini_core/src/nnue.rs
const FINAL_NNUE_GOD_COUNT: usize = 53;
const BOARD_FEATURES_COUNT: usize = 5 * 25;
const MATCHUP_BIT_FEATURES_COUNT: usize = FINAL_NNUE_GOD_COUNT * FINAL_NNUE_GOD_COUNT;
const TOTAL_BASE_FEATURES_COUNT: usize = BOARD_FEATURES_COUNT + MATCHUP_BIT_FEATURES_COUNT;
const WORKER_FEATURES_COUNT: usize = 25 * 4;
const PER_SIDE_MAIN_FEATURES_COUNT: usize =
    WORKER_FEATURES_COUNT + TOTAL_GOD_DATA_FEATURE_COUNT_FOR_NNUE;

const NORMALIZING_FEATURES_START: usize =
    TOTAL_BASE_FEATURES_COUNT + 2 * PER_SIDE_MAIN_FEATURES_COUNT;

const TOTAL_NUM_FEATURES_WITH_EXTRA_BITS: usize =
    NORMALIZING_FEATURES_START + 2 * FINAL_NNUE_GOD_COUNT;

// const TOTAL_NUM_FEATURES_WITH_EXTRA_BITS: usize = 2164;
// const NORMALIZING_FEATURES_START: usize = 2086;

#[repr(C)]
#[derive(Clone, Debug)]
struct Network<const FEATURES: usize> {
    pub feature_weights: [Accumulator; FEATURES],
    pub feature_bias: Accumulator,
    pub output_weights: [i16; HIDDEN_SIZE],
    pub output_bias: i16,
}

fn load_base_model() -> Box<Network<TOTAL_NUM_FEATURES_WITH_EXTRA_BITS>> {
    let bytes = std::fs::read("./models/batch5_full.bin").expect("Failed to read model binary");
    eprintln!(
        "Read {} bytes from model file. compared to expected: {}",
        bytes.len(),
        std::mem::size_of::<Network<TOTAL_NUM_FEATURES_WITH_EXTRA_BITS>>()
    );

    let mut model_clone = Box::new_uninit();
    unsafe {
        std::ptr::copy_nonoverlapping(
            bytes.as_ptr() as *const u8,
            model_clone.as_mut_ptr() as *mut u8,
            std::mem::size_of::<Network<TOTAL_NUM_FEATURES_WITH_EXTRA_BITS>>(),
        );
        return model_clone.assume_init();
    }
}

fn get_smaller_model(
    full_model: &Network<TOTAL_NUM_FEATURES_WITH_EXTRA_BITS>,
) -> Box<Network<NORMALIZING_FEATURES_START>> {
    let smaller_model = Box::new_uninit();
    unsafe {
        let mut smaller_model: Box<Network<NORMALIZING_FEATURES_START>> =
            smaller_model.assume_init();
        smaller_model
            .feature_weights
            .copy_from_slice(&full_model.feature_weights[0..NORMALIZING_FEATURES_START]);
        smaller_model.feature_bias = full_model.feature_bias;
        smaller_model.output_weights = full_model.output_weights;
        smaller_model.output_bias = full_model.output_bias;

        return smaller_model;
    }
}

fn main() {
    eprintln!(
        "TOTAL_NUM_FEATURES FROM NNUE (final): {}",
        TOTAL_NUM_FEATURES
    );
    eprintln!(
        "TOTAL_NUM_FEATURES (this script, with god bits): {}",
        TOTAL_NUM_FEATURES_WITH_EXTRA_BITS
    );
    eprintln!(
        "NORMALIZING_FEATURES_START (this script, no god bits): {}",
        NORMALIZING_FEATURES_START
    );

    let mut model_clone = load_base_model();

    for god1 in 0..FINAL_NNUE_GOD_COUNT {
        let g1_idx = NORMALIZING_FEATURES_START + god1;

        for god2 in 0..FINAL_NNUE_GOD_COUNT {
            let g2_idx = NORMALIZING_FEATURES_START + FINAL_NNUE_GOD_COUNT + god2;
            let matchup_feature_delta = (god1 * FINAL_NNUE_GOD_COUNT) + god2;
            let matchup_feature_final = matchup_feature_delta + MATCHUP_BIT_FEATURE_OFFSET as usize;

            for i in 0..HIDDEN_SIZE {
                model_clone.feature_weights[matchup_feature_final].vals.0[i] +=
                    model_clone.feature_weights[g1_idx].vals.0[i]
                        + model_clone.feature_weights[g2_idx].vals.0[i];
            }
        }
    }

    eprintln!("updated model in place");
    let smaller_model = get_smaller_model(&model_clone);
    eprintln!("created smaller model");

    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            smaller_model.as_ref() as *const Network<NORMALIZING_FEATURES_START> as *const u8,
            std::mem::size_of::<Network<NORMALIZING_FEATURES_START>>(),
        )
    };
    std::fs::write("matchup_bit_post_processed.bin", bytes).unwrap();
}

// cargo run -p santorini_core --bin post_process_model -r
