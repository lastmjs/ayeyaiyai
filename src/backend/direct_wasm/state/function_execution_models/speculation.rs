use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct FunctionSpeculationState {
    pub(in crate::backend::direct_wasm) static_semantics: FunctionStaticSemanticsState,
    pub(in crate::backend::direct_wasm) execution_context: FunctionExecutionContextState,
}
