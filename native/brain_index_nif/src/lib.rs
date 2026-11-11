//! BrainIndex native core. Design decisions live in `@meta/@adr`.

pub mod error;
pub mod hnsw;
pub mod index;
pub mod metric;
pub mod simd;
pub mod topk;

use error::Error;
use index::Index;
use metric::Metric;
use rustler::{Atom, Binary, Encoder, Env, ResourceArc, Term};
use std::sync::RwLock;

mod atoms {
    rustler::atoms! { ok }
}

pub struct IndexRes(RwLock<Index>);

#[rustler::resource_impl]
impl rustler::Resource for IndexRes {}

impl Encoder for Error {
    fn encode<'a>(&self, env: Env<'a>) -> Term<'a> {
        let reason = Atom::from_str(env, self.reason).expect("valid atom");
        (reason, self.message.as_str()).encode(env)
    }
}

type NifResult<T> = Result<T, Error>;

/// Decodes a little-endian f32 binary.
fn floats(bin: &Binary) -> NifResult<Vec<f32>> {
    if !bin.len().is_multiple_of(4) {
        return Err(Error::invalid_vector(
            "vector binary length must be a multiple of 4",
        ));
    }
    Ok(bin
        .as_slice()
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn poisoned() -> Error {
    Error::new("internal", "index lock poisoned")
}

#[rustler::nif]
fn nif_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[rustler::nif]
fn new(
    dim: usize,
    metric: String,
    m: usize,
    ef_construction: usize,
) -> NifResult<ResourceArc<IndexRes>> {
    let params = hnsw::Params {
        m,
        ef_construction,
        ..Default::default()
    };
    let idx = Index::with_params(dim, Metric::parse(&metric)?, params)?;
    Ok(ResourceArc::new(IndexRes(RwLock::new(idx))))
}

#[rustler::nif]
fn insert(res: ResourceArc<IndexRes>, id: u64, vector: Binary) -> NifResult<Atom> {
    let v = floats(&vector)?;
    res.0.write().map_err(|_| poisoned())?.insert(id, &v)?;
    Ok(atoms::ok())
}

#[rustler::nif]
fn size(res: ResourceArc<IndexRes>) -> NifResult<usize> {
    Ok(res.0.read().map_err(|_| poisoned())?.len())
}

#[rustler::nif(schedule = "DirtyCpu")]
fn exact_search(res: ResourceArc<IndexRes>, query: Binary, k: usize) -> NifResult<Vec<(u64, f64)>> {
    let q = floats(&query)?;
    let idx = res.0.read().map_err(|_| poisoned())?;
    Ok(idx
        .exact_search(&q, k, |_| true)?
        .into_iter()
        .map(|(id, d)| (id, d as f64))
        .collect())
}

#[rustler::nif(schedule = "DirtyCpu")]
fn search(
    res: ResourceArc<IndexRes>,
    query: Binary,
    k: usize,
    ef: usize,
) -> NifResult<Vec<(u64, f64)>> {
    let q = floats(&query)?;
    let idx = res.0.read().map_err(|_| poisoned())?;
    Ok(idx
        .search(&q, k, ef)?
        .into_iter()
        .map(|(id, d)| (id, d as f64))
        .collect())
}

rustler::init!("Elixir.BrainIndex.Native");
