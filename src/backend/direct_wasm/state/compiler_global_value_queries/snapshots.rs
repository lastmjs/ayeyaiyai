use super::super::*;

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn snapshot_global_binding_environment(
        &self,
    ) -> GlobalBindingEnvironment {
        GlobalBindingEnvironment {
            value_bindings: self.global_semantics.values.snapshot_value_bindings(),
            object_bindings: self.global_semantics.values.snapshot_object_bindings(),
        }
    }

    pub(in crate::backend::direct_wasm) fn snapshot_top_level_static_state(
        &self,
    ) -> (
        HashMap<String, Expression>,
        HashMap<String, ObjectValueBinding>,
    ) {
        self.global_semantics
            .values
            .snapshot_top_level_static_state()
    }
}
