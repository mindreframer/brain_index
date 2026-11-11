//! Stable error contract (ADR006). `reason` maps 1:1 to an Elixir atom.

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub reason: &'static str,
    pub message: String,
}

impl Error {
    pub fn new(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
    pub fn invalid_argument(m: impl Into<String>) -> Self {
        Self::new("invalid_argument", m)
    }
    pub fn invalid_vector(m: impl Into<String>) -> Self {
        Self::new("invalid_vector", m)
    }
    pub fn dimension_mismatch(expected: usize, got: usize) -> Self {
        Self::new(
            "dimension_mismatch",
            format!("expected {expected} dimensions, got {got}"),
        )
    }
}

pub type Result<T> = std::result::Result<T, Error>;
