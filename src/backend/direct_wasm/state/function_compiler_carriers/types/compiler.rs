use std::collections::HashMap;
use std::rc::Rc;

use super::*;

pub(in crate::backend::direct_wasm) struct FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) backend: FunctionCompilerBackend<'a>,
    pub(in crate::backend::direct_wasm) prepared_program: PreparedSharedProgramContext,
    pub(in crate::backend::direct_wasm) assigned_nonlocal_binding_results:
        Rc<HashMap<String, HashMap<String, Expression>>>,
    pub(in crate::backend::direct_wasm) state: FunctionCompilerState,
}

pub(in crate::backend::direct_wasm) struct FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) module_artifacts: &'a mut ModuleArtifactsState,
    pub(in crate::backend::direct_wasm) function_registry: &'a mut FunctionRegistryState,
    pub(in crate::backend::direct_wasm) test262: &'a mut Test262State,
    pub(in crate::backend::direct_wasm) global_semantics: GlobalStaticSemanticsSnapshot,
}
