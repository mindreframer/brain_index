//! Dot-product kernels: NEON on aarch64 (always available), AVX2+FMA on x86_64 when the CPU
//! supports it (runtime-detected), scalar otherwise. Idea from HyperspaceDB `hyperspace-core/simd.rs`
//! (MIT, YARlabs); the kernels here are written from scratch with four accumulators.

#[inline]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: NEON is mandatory on aarch64.
        unsafe { dot_neon(a, b) }
    }
    #[cfg(target_arch = "x86_64")]
    {
        if avx2_fma() {
            // SAFETY: guarded by runtime feature detection.
            return unsafe { dot_avx2(a, b) };
        }
        dot_scalar(a, b)
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        dot_scalar(a, b)
    }
}

#[inline]
pub fn dot_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn dot_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;
    let n = a.len().min(b.len());
    let (pa, pb) = (a.as_ptr(), b.as_ptr());
    let mut acc = [vdupq_n_f32(0.0); 4];
    let mut i = 0;
    while i + 16 <= n {
        for (j, acc_j) in acc.iter_mut().enumerate() {
            let o = i + 4 * j;
            *acc_j = vfmaq_f32(*acc_j, vld1q_f32(pa.add(o)), vld1q_f32(pb.add(o)));
        }
        i += 16;
    }
    while i + 4 <= n {
        acc[0] = vfmaq_f32(acc[0], vld1q_f32(pa.add(i)), vld1q_f32(pb.add(i)));
        i += 4;
    }
    let mut sum = vaddvq_f32(vaddq_f32(
        vaddq_f32(acc[0], acc[1]),
        vaddq_f32(acc[2], acc[3]),
    ));
    while i < n {
        sum += *pa.add(i) * *pb.add(i);
        i += 1;
    }
    sum
}

#[cfg(target_arch = "x86_64")]
fn avx2_fma() -> bool {
    use std::sync::OnceLock;
    static DETECTED: OnceLock<bool> = OnceLock::new();
    *DETECTED.get_or_init(|| is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma"))
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2,fma")]
unsafe fn dot_avx2(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::x86_64::*;
    let n = a.len().min(b.len());
    let (pa, pb) = (a.as_ptr(), b.as_ptr());
    let mut acc = [_mm256_setzero_ps(); 4];
    let mut i = 0;
    while i + 32 <= n {
        for (j, acc_j) in acc.iter_mut().enumerate() {
            let o = i + 8 * j;
            *acc_j = _mm256_fmadd_ps(
                _mm256_loadu_ps(pa.add(o)),
                _mm256_loadu_ps(pb.add(o)),
                *acc_j,
            );
        }
        i += 32;
    }
    while i + 8 <= n {
        acc[0] = _mm256_fmadd_ps(
            _mm256_loadu_ps(pa.add(i)),
            _mm256_loadu_ps(pb.add(i)),
            acc[0],
        );
        i += 8;
    }
    let s = _mm256_add_ps(_mm256_add_ps(acc[0], acc[1]), _mm256_add_ps(acc[2], acc[3]));
    let mut lanes = [0f32; 8];
    _mm256_storeu_ps(lanes.as_mut_ptr(), s);
    let mut sum: f32 = lanes.iter().sum();
    while i < n {
        sum += *pa.add(i) * *pb.add(i);
        i += 1;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simd_matches_scalar_for_all_tail_lengths() {
        for n in [
            0usize, 1, 3, 4, 7, 8, 15, 16, 17, 31, 32, 33, 129, 768, 1000,
        ] {
            let a: Vec<f32> = (0..n).map(|i| ((i * 7 % 13) as f32 - 6.0) / 7.0).collect();
            let b: Vec<f32> = (0..n).map(|i| ((i * 5 % 11) as f32 - 5.0) / 5.0).collect();
            let (fast, slow) = (dot(&a, &b), dot_scalar(&a, &b));
            assert!(
                (fast - slow).abs() <= 1e-5 * slow.abs().max(1.0),
                "n={n}: {fast} vs {slow}"
            );
        }
    }
}
