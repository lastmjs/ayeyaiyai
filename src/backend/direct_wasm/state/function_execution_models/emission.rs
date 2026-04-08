use super::*;

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct FunctionControlFlowState {
    pub(in crate::backend::direct_wasm) control_stack: Vec<()>,
    pub(in crate::backend::direct_wasm) loop_stack: Vec<LoopContext>,
    pub(in crate::backend::direct_wasm) break_stack: Vec<BreakContext>,
    pub(in crate::backend::direct_wasm) try_stack: Vec<TryContext>,
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct FunctionOutputState {
    pub(in crate::backend::direct_wasm) instructions: Vec<u8>,
}

#[derive(Clone, Default)]
pub(in crate::backend::direct_wasm) struct FunctionEmissionState {
    pub(in crate::backend::direct_wasm) output: FunctionOutputState,
    pub(in crate::backend::direct_wasm) control_flow: FunctionControlFlowState,
    pub(in crate::backend::direct_wasm) lexical_scopes: FunctionLexicalScopeState,
}
