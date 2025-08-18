#![allow(dead_code, unused_imports)]
#![feature(portable_simd)]

use colored::Colorize;
use rand::{rng, Rng};
use rand::seq::{IteratorRandom, SliceRandom};
use santorini_core::bitboard::BitBoard;
use santorini_core::board::{BoardState, NEIGHBOR_MAP, WRAPPING_NEIGHBOR_MAP};
use santorini_core::gods::generic::{
    IMPROVER_SENTINEL_SCORE, MOVE_IS_CHECK_MASK, MOVE_IS_WINNING_MASK,
};
use santorini_core::gods::{ALL_GODS_BY_ID, GodName};
use santorini_core::nnue::{
    self, Accumulator, FEATURE_LANES, HIDDEN_SIZE, MODEL, QB, TOTAL_FEATURES,
};
use santorini_core::placement::{get_all_placements, get_unique_placements};
use santorini_core::random_utils::GameStateFuzzer;
use santorini_core::square::Square;
use santorini_core::transposition_table::{LMRTable, TTEntry, TTValue};
use santorini_core::utils::print_cpu_arch;

use std::simd;
use std::simd::Simd;
use std::simd::num::SimdInt;

const LANES: usize = 8;

/// Multiply two i16 slices elementwise and sum the result using SIMD.
/// Panics if lengths do not match.
fn simd_mul_sum_i16(a: &[i16], b: &[i16]) -> i32 {
    assert_eq!(a.len(), b.len());
    let mut simd_sum = Simd::<i32, LANES>::splat(0);

    let mut i = 0;
    while i + LANES <= a.len() {
        let va = Simd::<i16, LANES>::from_slice(&a[i..i + LANES]);
        let vb = Simd::<i16, LANES>::from_slice(&b[i..i + LANES]);
        // Multiply as i32 to avoid overflow on sum
        let prod = va.cast::<i32>() * vb.cast::<i32>();
        simd_sum += prod;
        i += LANES;
    }

    // Reduce SIMD register to scalar sum
    let mut sum = simd_sum.reduce_sum();

    // Handle remainder
    for j in i..a.len() {
        sum += a[j] as i32 * b[j] as i32;
    }
    sum
}

struct HiddenVis {
    hidden_idx: usize,
    output_weight: i16,
    feature_bias: i16,
    feature_weights: [i16; TOTAL_FEATURES],
}

impl PartialEq for HiddenVis {
    fn eq(&self, other: &Self) -> bool {
        self.hidden_idx == other.hidden_idx
    }
}

impl Eq for HiddenVis {}

impl Ord for HiddenVis {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.output_weight
            .cmp(&other.output_weight)
            .then(self.feature_bias.cmp(&other.feature_bias))
            .then(self.hidden_idx.cmp(&other.hidden_idx))
    }
}

impl PartialOrd for HiddenVis {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn vis_feature_grid() {
    todo!()
}

fn get_hidden_vis() -> Vec<HiddenVis> {
    let mut res = Vec::new();
    for i in 0..HIDDEN_SIZE {
        let mut feature_weights = [0_16; TOTAL_FEATURES];
        for f in 0..TOTAL_FEATURES {
            feature_weights[f] = MODEL.feature_weights[f].vals[i];
        }

        res.push(HiddenVis {
            hidden_idx: i,
            output_weight: MODEL.output_weights[i],
            feature_bias: MODEL.feature_bias.vals[i],
            feature_weights,
        });
    }

    res
}

// Map value from the domain (-scale, +scale), to (0, 1)
fn scale_value(value: i16, range: i16) -> f64 {
    let mut f_value = (value + range) as f64;
    f_value /= (2 * range) as f64;
    f_value
}

const HIDDEN_SCALE_VAL: i16 = 500;

fn hidden_weight_to_color(weight: i16) -> colored::Color {
    let gradient = colorous::VIRIDIS;
    let colorous::Color { r, g, b } =
        gradient.eval_continuous(scale_value(weight, HIDDEN_SCALE_VAL));
    let color = colored::Color::TrueColor { r, g, b };
    color
}

fn print_hidden(hidden: &HiddenVis) {
    for r in 0..5 {
        for o in 0..3 {
            print!("{}", 5 - r);
            for h in 0..5 {
                for c in 0..5 {
                    let fidx = (r * 5 + c) * 5 + h + 125 * o;
                    let back = hidden_weight_to_color(hidden.feature_weights[fidx]);
                    // let square = '\u{2580}'.to_string().on_color(back).color(front);
                    let square = ' '.to_string().on_color(back);
                    print!("{square}");
                }
                print!(" ");
                if o > 0 && h >= 2 {
                    break;
                }
            }
            if o < 2 {
                print!("|");
            }
        }
        println!("");
    }
    for _h in 0..5 {
        print!(" ABCDE");
    }
    for _ in 0..2 {
        print!("  ");
        for _h in 0..3 {
            print!(" ABCDE");
        }
    }
    println!("");
}

fn feature_weight_spread() -> Vec<i16> {
    let mut res = Vec::new();

    for acc in &MODEL.feature_weights {
        for weight in acc.vals.0.iter() {
            res.push(*weight);
        }
    }

    res.sort();

    res
}

fn nnue_analysis() {
    // println!("{:?}", feature_weight_spread());

    let mut hidden_layers = get_hidden_vis();
    hidden_layers.sort();
    for h in hidden_layers {
        println!(
            "Hidden id: {} weight: {} bias: {}",
            h.hidden_idx, h.output_weight, h.feature_bias
        );
        print_hidden(&h);
    }

    // println!("output bias: {}", MODEL.output_bias);
    // let mut output_weights = MODEL.output_weights;
    // output_weights.sort();

    // println!("Output weights: {:?}", output_weights);

    // let mut feature_bias = MODEL.feature_bias.vals.0;
    // feature_bias.sort();
    // println!("Feature bias weights: {:?}", feature_bias);
}

fn tt_randomness_check() {
    const TT_SIZE: usize = 10_000_019;
    let mut tt = vec![false; TT_SIZE];
    let mut rng = rand::rng();

    let counts = 2_700_000;
    for _ in 0..counts {
        let slot = rng.random_range(0..TT_SIZE);
        tt[slot] = true;
    }

    let mut count: usize = 0;
    for item in &tt {
        count += *item as usize;
    }
    let collide = counts - count;

    let fill_pct = count as f32 / TT_SIZE as f32;
    eprintln!(
        "{} / {} x {} = {:.2}. {} collisions",
        count, TT_SIZE, counts, fill_pct, collide
    );
}

fn test_improvers() {
    let game_state_fuzzer = GameStateFuzzer::new(5);

    for state in game_state_fuzzer {
        let mortal = GodName::Mortal.to_power();
        let mut actions = mortal.get_moves_for_search(&state.board, state.board.current_player);
        actions.sort_by_key(|a| -a.score);

        state.board.print_to_console();

        for action in actions {
            let mut board = state.board.clone();
            let improving_string = if action.score == IMPROVER_SENTINEL_SCORE {
                "IMPROVER"
            } else {
                "QUIET"
            };
            println!("{}: {:?}", improving_string, action.action);
            mortal.make_move(&mut board, action.action);
            board.print_to_console();
        }
    }
}

fn random_matchup() {
    let god_names = ALL_GODS_BY_ID
        .iter()
        .map(|f| f.god_name)
        .filter(|f| *f != GodName::Mortal)
        .collect::<Vec<GodName>>();
    let choose = god_names
        .into_iter()
        .choose_multiple(&mut rand::rng(), 2)
        .into_iter()
        .collect::<Vec<_>>();
    println!("{:?}", choose);
}

fn _print_lmr_table() {
    let lmr = LMRTable::new();
    for (i, row) in lmr.table.iter().enumerate() {
        eprintln!("{i}: {:?}", row);
    }
}

fn print_hashing_randoms(size: usize) {
    let mut rng = rng();
    let random_numbers = (0..size)
        .map(|_| rng.random_range(0..u64::MAX))
        .collect::<Vec<_>>();

    eprintln!("{:?}", random_numbers);
}

fn _print_neighbor_map() {
    for i in 0..25 {
        let source = BitBoard::as_mask(Square::from(i));
        let ns = NEIGHBOR_MAP[i];

        println!("{source}");
        println!("{ns}");
    }
}

fn main() {
    _print_neighbor_map();
    // _print_lmr_table();

    // print_hashing_randoms(32);
    // random_matchup();

    // println!("{:b}", MOVE_IS_WINNING_MASK);
    // println!("{:b}", MOVE_IS_CHECK_MASK);
    // test_improvers();
    // println!("{}", size_of::<TTValue>());
    // println!("{}", size_of::<Option<TTValue>>());

    // tt_randomness_check();
    // nnue_analysis();

    // print_cpu_arch();

    // let a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    // let b = vec![10, 9, 8, 7, 6, 5, 4, 3, 2, 1];
    // let result = simd_mul_sum_i16(&a, &b);
    // println!("Sum of products: {}", result); // Should print 220
}
