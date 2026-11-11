//! Distance metrics (ADR003). Ported in spirit from HyperspaceDB `hyperspace-core` (MIT, YARlabs).

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Vectors are L2-normalised on insert/query; distance = 1 - dot.
    Cosine,
    /// Hyperboloid points, time component first; distance = acosh(-<a,b>_L).
    Lorentz,
}

impl Metric {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "cosine" => Ok(Metric::Cosine),
            "lorentz" => Ok(Metric::Lorentz),
            other => Err(Error::invalid_argument(format!("unknown metric {other:?}"))),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Metric::Cosine => "cosine",
            Metric::Lorentz => "lorentz",
        }
    }

    /// Validates and normalises a vector in place so `distance` can assume canonical form.
    pub fn prepare(self, v: &mut [f32]) -> Result<()> {
        if v.iter().any(|x| !x.is_finite()) {
            return Err(Error::invalid_vector("vector contains NaN or infinity"));
        }
        match self {
            Metric::Cosine => {
                let norm = dot(v, v).sqrt();
                if norm == 0.0 {
                    return Err(Error::invalid_vector("zero vector cannot be normalised"));
                }
                v.iter_mut().for_each(|x| *x /= norm);
            }
            Metric::Lorentz => {
                if v.len() < 2 || v[0] < 1.0 - 1e-4 {
                    return Err(Error::invalid_vector(
                        "lorentz vector needs x0 >= 1 and dim >= 2",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Distance between a prepared query and a stored (mapped, possibly unaligned) vector.
    #[inline]
    pub fn distance_bytes(self, q: &[f32], b: &[u8]) -> f32 {
        match self {
            Metric::Cosine => 1.0 - crate::simd::dot_bytes(q, b),
            Metric::Lorentz => {
                let b0 = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                let inner = crate::simd::dot_bytes(&q[1..], &b[4..]) - q[0] * b0;
                (-inner).max(1.0).acosh()
            }
        }
    }

    #[inline]
    pub fn distance(self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            Metric::Cosine => 1.0 - dot(a, b),
            Metric::Lorentz => {
                let inner = dot(&a[1..], &b[1..]) - a[0] * b[0];
                (-inner).max(1.0).acosh()
            }
        }
    }
}

#[inline]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    crate::simd::dot(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_distance_of_normalised_vectors() {
        let m = Metric::Cosine;
        let (mut a, mut b) = (vec![3.0, 4.0], vec![4.0, 3.0]);
        m.prepare(&mut a).unwrap();
        m.prepare(&mut b).unwrap();
        assert!((a[0] - 0.6).abs() < 1e-6);
        assert!((m.distance(&a, &b) - (1.0 - 0.96)).abs() < 1e-6);
        assert!(m.distance(&a, &a).abs() < 1e-6);
    }

    #[test]
    fn lorentz_distance_matches_closed_form() {
        // Points expmap0(r * e1): x = (cosh r, sinh r, 0). Distance between r=0.5 and r=1.5 is 1.0.
        let p = |r: f32| vec![r.cosh(), r.sinh(), 0.0];
        let d = Metric::Lorentz.distance(&p(0.5), &p(1.5));
        assert!((d - 1.0).abs() < 1e-4, "{d}");
        assert!(Metric::Lorentz.distance(&p(0.7), &p(0.7)) < 2e-3);
    }

    #[test]
    fn rejects_invalid_vectors() {
        assert_eq!(
            Metric::Cosine.prepare(&mut [0.0, 0.0]).unwrap_err().reason,
            "invalid_vector"
        );
        assert_eq!(
            Metric::Cosine
                .prepare(&mut [f32::NAN, 1.0])
                .unwrap_err()
                .reason,
            "invalid_vector"
        );
        assert_eq!(
            Metric::Lorentz.prepare(&mut [0.5, 0.1]).unwrap_err().reason,
            "invalid_vector"
        );
        assert!(Metric::parse("l2").is_err());
    }
}
