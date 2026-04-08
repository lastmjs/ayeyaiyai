use super::super::{FunctionCompiler, StaticResolutionEnvironment};
use crate::ir::hir::Expression;
use std::collections::HashMap;

impl FunctionCompiler<'_> {
    pub(in crate::backend::direct_wasm) fn assigned_nonlocal_binding_results(
        &self,
        function_name: &str,
    ) -> Option<&HashMap<String, Expression>> {
        self.assigned_nonlocal_binding_results.get(function_name)
    }
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn snapshot_static_resolution_environment(
        &self,
    ) -> StaticResolutionEnvironment {
        let global_bindings = self
            .prepared_program
            .required_shared_global_binding_environment();
        self.state
            .snapshot_static_resolution_environment(global_bindings)
    }

    pub(in crate::backend::direct_wasm) fn snapshot_static_resolution_environment_with_local_bindings(
        &self,
        local_bindings: HashMap<String, Expression>,
    ) -> StaticResolutionEnvironment {
        let global_bindings = self
            .prepared_program
            .required_shared_global_binding_environment();
        self.state
            .snapshot_static_resolution_environment_with_local_bindings(
                global_bindings,
                local_bindings,
            )
    }

    pub(in crate::backend::direct_wasm) fn snapshot_static_resolution_environment_without_locals(
        &self,
    ) -> StaticResolutionEnvironment {
        self.snapshot_static_resolution_environment_with_local_bindings(HashMap::new())
    }
}
