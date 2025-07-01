#![feature(portable_simd)]

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

fn main() {
    print_cpu_arch();

    let a = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let b = vec![10, 9, 8, 7, 6, 5, 4, 3, 2, 1];
    let result = simd_mul_sum_i16(&a, &b);
    println!("Sum of products: {}", result); // Should print 220
}
