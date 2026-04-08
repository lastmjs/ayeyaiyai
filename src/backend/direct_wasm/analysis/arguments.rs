use super::*;

#[path = "arguments/return_detection.rs"]
mod return_detection;
#[path = "arguments/returned_effects.rs"]
mod returned_effects;
#[path = "arguments/usage.rs"]
mod usage;

pub(in crate::backend::direct_wasm) use return_detection::*;
pub(in crate::backend::direct_wasm) use returned_effects::*;
pub(in crate::backend::direct_wasm) use usage::*;
