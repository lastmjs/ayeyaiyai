use super::super::*;

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn new(
        module_artifacts: &'a mut ModuleArtifactsState,
        function_registry: &'a mut FunctionRegistryState,
        test262: &'a mut Test262State,
        global_semantics: GlobalStaticSemanticsSnapshot,
    ) -> FunctionCompilerBackend<'a> {
        Self {
            module_artifacts,
            function_registry,
            test262,
            global_semantics,
        }
    }

    pub(in crate::backend::direct_wasm) fn intern_string(&mut self, bytes: Vec<u8>) -> (u32, u32) {
        self.module_artifacts.intern_string(bytes)
    }
}
