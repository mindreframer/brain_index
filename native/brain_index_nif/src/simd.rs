//! Dot-product kernel. Scalar for now; SIMD variants arrive in EPIC006.

#[inline]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
