#[path = "access_impls.rs"]
mod access_impls;
#[path = "access_trait.rs"]
mod access_trait;
#[path = "access_types.rs"]
mod access_types;

pub(in crate::backend::direct_wasm) use access_types::GlobalSemanticState;
