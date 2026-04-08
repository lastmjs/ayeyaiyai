#[path = "function_execution_models.rs"]
mod execution;
#[path = "function_static_semantics.rs"]
mod static_semantics;

pub(in crate::backend::direct_wasm) use execution::{
    FunctionEmissionState, FunctionExecutionContextState, FunctionLexicalScopeState,
    FunctionSpeculationState,
};
pub(in crate::backend::direct_wasm) use static_semantics::FunctionStaticSemanticsState;
pub(in crate::backend::direct_wasm) use static_semantics::{
    FunctionArraySemanticsState, FunctionObjectSemanticsState, FunctionValueSemanticsState,
};
