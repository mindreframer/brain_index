//! BrainIndex native core. See @meta/@adr for design decisions.

#[rustler::nif]
fn nif_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

rustler::init!("Elixir.BrainIndex.Native");
