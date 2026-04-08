use super::*;
use std::collections::HashMap;

impl FunctionCompilerState {
    pub(in crate::backend::direct_wasm) fn snapshot_static_binding_metadata(
        &self,
    ) -> FunctionStaticBindingMetadataSnapshot {
        self.speculation
            .static_semantics
            .snapshot_static_binding_metadata()
    }

    pub(in crate::backend::direct_wasm) fn restore_static_binding_metadata(
        &mut self,
        snapshot: FunctionStaticBindingMetadataSnapshot,
    ) {
        self.speculation
            .static_semantics
            .restore_static_binding_metadata(snapshot);
    }

    pub(in crate::backend::direct_wasm) fn snapshot_static_resolution_environment(
        &self,
        global_bindings: &SharedGlobalBindingEnvironment,
    ) -> StaticResolutionEnvironment {
        self.speculation
            .static_semantics
            .snapshot_static_resolution_environment(global_bindings)
    }

    pub(in crate::backend::direct_wasm) fn snapshot_static_resolution_environment_with_local_bindings(
        &self,
        global_bindings: &SharedGlobalBindingEnvironment,
        local_bindings: HashMap<String, Expression>,
    ) -> StaticResolutionEnvironment {
        self.speculation
            .static_semantics
            .snapshot_static_resolution_environment_with_local_bindings(
                global_bindings,
                local_bindings,
            )
    }

    pub(in crate::backend::direct_wasm) fn snapshot_local_static_binding(
        &self,
        name: &str,
    ) -> LocalStaticBindingSnapshot {
        self.speculation
            .static_semantics
            .snapshot_local_static_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn set_local_static_binding(
        &mut self,
        name: &str,
        value: Expression,
        array: Option<ArrayValueBinding>,
        object: Option<ObjectValueBinding>,
        kind: Option<StaticValueKind>,
    ) {
        self.speculation
            .static_semantics
            .set_local_static_binding(name, value, array, object, kind);
    }

    pub(in crate::backend::direct_wasm) fn restore_local_static_binding(
        &mut self,
        snapshot: LocalStaticBindingSnapshot,
    ) {
        self.speculation
            .static_semantics
            .restore_local_static_binding(snapshot);
    }
}
