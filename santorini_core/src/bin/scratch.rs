#![allow(dead_code, unused_imports)]
#![feature(portable_simd)]

use colored::Colorize;
use santorini_core::nnue::{self, Accumulator, FEATURE_LANES, FEATURES, HIDDEN_SIZE, MODEL, QB};
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
    feature_weights: [i16; FEATURES],
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
        let mut feature_weights = [0_16; FEATURES];
        for f in 0..FEATURES {
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

fn main() {
    nnue_analysis();

    // print_cpu_arch();

    // let a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    // let b = vec![10, 9, 8, 7, 6, 5, 4, 3, 2, 1];
    // let result = simd_mul_sum_i16(&a, &b);
    // println!("Sum of products: {}", result); // Should print 220
}
