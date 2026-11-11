//! BrainIndex native core. Design decisions live in `@meta/@adr`; this file is the NIF boundary
//! (ADR006): decode inputs, take the lock, contain panics, encode tagged results.

pub mod bitset;
pub mod error;
pub mod hnsw;
pub mod index;
pub mod metric;
pub mod simd;
pub mod snapshot;
pub mod topk;

use error::Error;
use index::{Index, SearchOpts};
use metric::Metric;
use rustler::{Atom, Binary, Encoder, Env, NewBinary, ResourceArc, Term};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
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
type MetaPairs = Vec<(String, String)>;
type FilterSpec = Vec<(String, Vec<String>)>;
/// (k, ef, exact, brute_force_threshold)
type SearchSpec = (usize, usize, bool, usize);
/// (dim, metric, size, deleted, m, ef_construction)
type InfoSpec = (usize, &'static str, usize, usize, usize, usize);

/// Runs a NIF body, converting any panic into a tagged `internal` error.
fn guard<T>(f: impl FnOnce() -> NifResult<T>) -> NifResult<T> {
    catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err(Error::new("internal", "native panic contained")))
}

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

fn read(res: &IndexRes) -> NifResult<std::sync::RwLockReadGuard<'_, Index>> {
    res.0
        .read()
        .map_err(|_| Error::new("internal", "index lock poisoned"))
}

fn write(res: &IndexRes) -> NifResult<std::sync::RwLockWriteGuard<'_, Index>> {
    res.0
        .write()
        .map_err(|_| Error::new("internal", "index lock poisoned"))
}

fn wrap(idx: Index) -> ResourceArc<IndexRes> {
    ResourceArc::new(IndexRes(RwLock::new(idx)))
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
    guard(|| {
        let params = hnsw::Params {
            m,
            ef_construction,
            ..Default::default()
        };
        Ok(wrap(Index::with_params(
            dim,
            Metric::parse(&metric)?,
            params,
        )?))
    })
}

#[rustler::nif(schedule = "DirtyCpu")]
fn insert(res: ResourceArc<IndexRes>, id: u64, vector: Binary, meta: MetaPairs) -> NifResult<Atom> {
    guard(|| {
        let v = floats(&vector)?;
        write(&res)?.insert(id, &v, meta)?;
        Ok(atoms::ok())
    })
}

#[rustler::nif(schedule = "DirtyCpu")]
fn insert_many(
    res: ResourceArc<IndexRes>,
    items: Vec<(u64, Binary, MetaPairs)>,
) -> NifResult<Atom> {
    guard(|| {
        let decoded = items
            .into_iter()
            .map(|(id, v, meta)| floats(&v).map(|v| (id, v, meta)))
            .collect::<NifResult<Vec<_>>>()?;
        write(&res)?.insert_many(decoded)?;
        Ok(atoms::ok())
    })
}

#[rustler::nif(schedule = "DirtyCpu")]
fn search(
    res: ResourceArc<IndexRes>,
    query: Binary,
    filter: FilterSpec,
    opts: SearchSpec,
) -> NifResult<Vec<(u64, f64)>> {
    guard(|| {
        let q = floats(&query)?;
        let (k, ef, exact, brute_force_threshold) = opts;
        let o = SearchOpts {
            k,
            ef,
            exact,
            brute_force_threshold,
        };
        let hits = read(&res)?.search(&q, &filter, &o)?;
        Ok(hits.into_iter().map(|(id, d)| (id, d as f64)).collect())
    })
}

#[rustler::nif]
fn delete(res: ResourceArc<IndexRes>, id: u64) -> NifResult<bool> {
    guard(|| Ok(write(&res)?.delete(id)))
}

#[rustler::nif]
fn info(res: ResourceArc<IndexRes>) -> NifResult<InfoSpec> {
    guard(|| {
        let idx = read(&res)?;
        let p = &idx.graph.params;
        Ok((
            idx.dim,
            idx.metric.name(),
            idx.len(),
            idx.deleted(),
            p.m,
            p.ef_construction,
        ))
    })
}

#[rustler::nif(schedule = "DirtyIo")]
fn save(res: ResourceArc<IndexRes>, path: String) -> NifResult<Atom> {
    guard(|| {
        snapshot::save(&*read(&res)?, Path::new(&path))?;
        Ok(atoms::ok())
    })
}

#[rustler::nif(schedule = "DirtyIo")]
fn load(path: String) -> NifResult<ResourceArc<IndexRes>> {
    guard(|| Ok(wrap(snapshot::load(Path::new(&path))?)))
}

#[rustler::nif(schedule = "DirtyCpu")]
fn to_binary<'a>(env: Env<'a>, res: ResourceArc<IndexRes>) -> NifResult<Binary<'a>> {
    guard(|| {
        let bytes = snapshot::encode(&*read(&res)?);
        let mut out = NewBinary::new(env, bytes.len());
        out.as_mut_slice().copy_from_slice(&bytes);
        Ok(out.into())
    })
}

#[rustler::nif(schedule = "DirtyCpu")]
fn from_binary(bytes: Binary) -> NifResult<ResourceArc<IndexRes>> {
    guard(|| Ok(wrap(snapshot::decode(bytes.as_slice())?)))
}

/// Makes directory entries (new files, renames) durable.
#[rustler::nif(schedule = "DirtyIo")]
fn fsync_dir(path: String) -> NifResult<Atom> {
    guard(|| {
        snapshot::fsync_dir(Path::new(&path))?;
        Ok(atoms::ok())
    })
}

rustler::init!("Elixir.BrainIndex.Native");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_contains_panics() {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r: NifResult<()> = guard(|| panic!("boom"));
        std::panic::set_hook(prev);
        assert_eq!(r.unwrap_err().reason, "internal");
    }
}
