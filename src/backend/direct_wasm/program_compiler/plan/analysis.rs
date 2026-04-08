use super::*;

#[path = "analysis/function_inputs.rs"]
mod function_inputs;
pub(in crate::backend::direct_wasm) use function_inputs::*;

#[path = "analysis/prepared_analysis.rs"]
mod prepared_analysis;
pub(in crate::backend::direct_wasm) use prepared_analysis::*;
