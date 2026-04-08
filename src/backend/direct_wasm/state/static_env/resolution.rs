#[path = "resolution_ops.rs"]
mod ops;
#[path = "resolution_types.rs"]
mod types;

pub(in crate::backend::direct_wasm) use types::StaticResolutionEnvironment;
