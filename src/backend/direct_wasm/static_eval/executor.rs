#[path = "executor/contract.rs"]
mod contract;
pub(in crate::backend::direct_wasm) use contract::*;

#[path = "executor/source_impl.rs"]
mod source_impl;
