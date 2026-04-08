use super::*;

mod binding_environment;
mod global_evaluation;
mod resolution;

pub(in crate::backend::direct_wasm) use binding_environment::{
    GlobalBindingEnvironment, SharedGlobalBindingEnvironment,
};
pub(in crate::backend::direct_wasm) use global_evaluation::GlobalStaticEvaluationEnvironment;
pub(in crate::backend::direct_wasm) use resolution::StaticResolutionEnvironment;
