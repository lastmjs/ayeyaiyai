use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedModuleLayout {
    pub(in crate::backend::direct_wasm) user_type_arities: Vec<u32>,
    pub(in crate::backend::direct_wasm) user_functions: Vec<UserFunction>,
    pub(in crate::backend::direct_wasm) global_binding_count: u32,
    pub(in crate::backend::direct_wasm) implicit_global_binding_count: u32,
    pub(in crate::backend::direct_wasm) runtime_prototype_binding_count: u32,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedBackendProgram {
    pub(in crate::backend::direct_wasm) start: PreparedStartFunction,
    pub(in crate::backend::direct_wasm) user_functions: Vec<PreparedUserFunctionCompilation>,
    pub(in crate::backend::direct_wasm) analysis: PreparedProgramAnalysis,
    pub(in crate::backend::direct_wasm) module_layout: PreparedModuleLayout,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct EmittedModuleArtifacts {
    pub(in crate::backend::direct_wasm) string_data: Vec<(u32, Vec<u8>)>,
    pub(in crate::backend::direct_wasm) next_data_offset: u32,
    pub(in crate::backend::direct_wasm) int_min_ptr: u32,
    pub(in crate::backend::direct_wasm) int_min_len: u32,
}

pub(in crate::backend::direct_wasm) struct EmittedBackendProgram {
    pub(in crate::backend::direct_wasm) compiled_start: CompiledFunction,
    pub(in crate::backend::direct_wasm) compiled_functions: Vec<CompiledFunction>,
    pub(in crate::backend::direct_wasm) module_layout: PreparedModuleLayout,
    pub(in crate::backend::direct_wasm) artifacts: EmittedModuleArtifacts,
}
