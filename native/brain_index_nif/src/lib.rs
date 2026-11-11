//! BrainIndex native core. Design decisions live in `@meta/@adr`; this file is the NIF boundary
//! (ADR006): decode inputs, contain panics, encode tagged results. Storage is LMDB (ADR009).

pub mod bitset;
pub mod error;
pub mod hnsw;
pub mod index;
pub mod metric;
pub mod simd;
pub mod store;
pub mod topk;

use error::Error;
use index::{Config, Index, SearchOpts};
use metric::Metric;
use rustler::{Atom, Binary, Encoder, Env, NewBinary, ResourceArc, Term};
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

mod atoms {
    rustler::atoms! { ok, nil }
}

pub struct IndexRes(Arc<Index>);

/// Open environments in this OS process: LMDB allows one Env per directory, so `open` on an
/// already-open directory returns the same index.
static OPEN: Mutex<Option<HashMap<PathBuf, Weak<Index>>>> = Mutex::new(None);

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
/// (k, ef, exact, brute_force_threshold, payload)
type SearchSpec = (usize, usize, bool, usize, bool);
/// (dim, metric, m, ef_construction, map_size, sync)
type ConfigSpec = (usize, String, usize, usize, usize, bool);
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
    Ok(store::decode_f32s(bin.as_slice()))
}

fn config(spec: ConfigSpec) -> NifResult<Config> {
    let (dim, metric, m, ef_construction, map_size, sync) = spec;
    Ok(Config {
        dim,
        metric: Metric::parse(&metric)?,
        m,
        ef_construction,
        map_size,
        sync,
    })
}

fn bin<'a>(env: Env<'a>, bytes: &[u8]) -> Term<'a> {
    let mut out = NewBinary::new(env, bytes.len());
    out.as_mut_slice().copy_from_slice(bytes);
    Binary::from(out).encode(env)
}

#[rustler::nif]
fn nif_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[rustler::nif(schedule = "DirtyIo")]
fn open(dir: String, spec: ConfigSpec) -> NifResult<ResourceArc<IndexRes>> {
    guard(|| {
        let cfg = config(spec)?;
        std::fs::create_dir_all(&dir).map_err(|e| Error::new("io", e.to_string()))?;
        let key = std::fs::canonicalize(&dir).map_err(|e| Error::new("io", e.to_string()))?;
        let mut open = OPEN
            .lock()
            .map_err(|_| Error::new("internal", "registry poisoned"))?;
        let open = open.get_or_insert_with(HashMap::new);
        open.retain(|_, w| w.strong_count() > 0);
        if let Some(existing) = open.get(&key).and_then(Weak::upgrade) {
            existing.check_config(&cfg)?;
            return Ok(ResourceArc::new(IndexRes(existing)));
        }
        let index = Arc::new(Index::open(&key, &cfg)?);
        open.insert(key, Arc::downgrade(&index));
        Ok(ResourceArc::new(IndexRes(index)))
    })
}

#[rustler::nif(schedule = "DirtyIo")]
fn temp(spec: ConfigSpec) -> NifResult<ResourceArc<IndexRes>> {
    guard(|| {
        Ok(ResourceArc::new(IndexRes(Arc::new(Index::temp(&config(
            spec,
        )?)?))))
    })
}

#[rustler::nif(schedule = "DirtyIo")]
fn insert_many(
    res: ResourceArc<IndexRes>,
    items: Vec<(u64, Binary, MetaPairs, Option<Binary>)>,
) -> NifResult<Atom> {
    guard(|| {
        let decoded = items
            .into_iter()
            .map(|(id, v, meta, payload)| {
                floats(&v).map(|v| (id, v, meta, payload.map(|p| p.as_slice().to_vec())))
            })
            .collect::<NifResult<Vec<_>>>()?;
        res.0.insert_many(decoded)?;
        Ok(atoms::ok())
    })
}

#[rustler::nif(schedule = "DirtyCpu")]
fn search<'a>(
    env: Env<'a>,
    res: ResourceArc<IndexRes>,
    query: Binary,
    filter: FilterSpec,
    opts: SearchSpec,
) -> NifResult<Vec<Term<'a>>> {
    guard(|| {
        let q = floats(&query)?;
        let (k, ef, exact, brute_force_threshold, payload) = opts;
        let o = SearchOpts {
            k,
            ef,
            exact,
            brute_force_threshold,
            payload,
        };
        let hits = res.0.search(&q, &filter, &o)?;
        Ok(hits
            .into_iter()
            .map(|(id, d, p)| match (payload, p) {
                (false, _) => (id, d as f64).encode(env),
                (true, Some(p)) => (id, d as f64, bin(env, &p)).encode(env),
                (true, None) => (id, d as f64, atoms::nil()).encode(env),
            })
            .collect())
    })
}

#[rustler::nif(schedule = "DirtyIo")]
fn delete(res: ResourceArc<IndexRes>, id: u64) -> NifResult<bool> {
    guard(|| res.0.delete(id))
}

#[rustler::nif(schedule = "DirtyIo")]
fn get_payload<'a>(env: Env<'a>, res: ResourceArc<IndexRes>, id: u64) -> NifResult<Term<'a>> {
    guard(|| {
        Ok(res
            .0
            .get_payload(id)?
            .map_or(atoms::nil().encode(env), |p| bin(env, &p)))
    })
}

#[rustler::nif]
fn info(res: ResourceArc<IndexRes>) -> NifResult<InfoSpec> {
    guard(|| {
        let (dim, metric, live, deleted, m, ef_c) = res.0.info()?;
        Ok((dim, metric.name(), live, deleted, m, ef_c))
    })
}

#[rustler::nif(schedule = "DirtyIo")]
fn backup(res: ResourceArc<IndexRes>, dir: String) -> NifResult<Atom> {
    guard(|| {
        res.0.backup(Path::new(&dir))?;
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
