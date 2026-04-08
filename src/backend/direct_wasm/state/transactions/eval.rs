#[path = "eval/isolated_indirect.rs"]
mod isolated_indirect;
#[path = "eval/static_bindings.rs"]
mod static_bindings;

pub(in crate::backend::direct_wasm) use isolated_indirect::IsolatedIndirectEvalTransaction;
pub(in crate::backend::direct_wasm) use static_bindings::StaticBindingMetadataTransaction;
