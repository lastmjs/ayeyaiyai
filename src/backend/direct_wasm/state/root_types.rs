use super::{FunctionRegistryState, GlobalSemanticState, ModuleArtifactsState, Test262State};

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) state: CompilerState,
}

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct CompilerState {
    pub(in crate::backend::direct_wasm) module_artifacts: ModuleArtifactsState,
    pub(in crate::backend::direct_wasm) function_registry: FunctionRegistryState,
    pub(in crate::backend::direct_wasm) global_semantics: GlobalSemanticState,
    pub(in crate::backend::direct_wasm) test262: Test262State,
}

#[derive(Clone, Copy)]
pub(in crate::backend::direct_wasm) struct ImplicitGlobalBinding {
    pub(in crate::backend::direct_wasm) value_index: u32,
    pub(in crate::backend::direct_wasm) present_index: u32,
}
