use super::*;

mod locals;
mod state;

pub(in crate::backend::direct_wasm) use locals::FunctionRuntimeLocalsState;
pub(in crate::backend::direct_wasm) use state::FunctionRuntimeState;
