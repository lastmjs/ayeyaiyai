use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn next_available_global_index(&self) -> u32 {
        let names_next_index = self
            .state
            .global_semantics
            .names
            .next_allocated_global_index();
        self.state
            .global_semantics
            .values
            .max_runtime_prototype_global_index()
            .map(|index| index + 1)
            .unwrap_or(names_next_index)
            .max(names_next_index)
    }

    pub(in crate::backend::direct_wasm) fn mark_global_array_with_runtime_state(
        &mut self,
        name: &str,
    ) {
        self.state
            .global_semantics
            .values
            .mark_array_with_runtime_state(name);
    }

    pub(in crate::backend::direct_wasm) fn runtime_prototype_binding_names(&self) -> Vec<String> {
        self.state
            .global_semantics
            .values
            .runtime_prototype_binding_names()
    }

    pub(in crate::backend::direct_wasm) fn set_runtime_prototype_binding_global_index(
        &mut self,
        name: &str,
        global_index: u32,
    ) {
        self.state
            .global_semantics
            .values
            .set_runtime_prototype_binding_global_index(name, global_index);
    }

    pub(in crate::backend::direct_wasm) fn update_global_object_prototype_expression(
        &mut self,
        name: &str,
        prototype: Expression,
    ) {
        self.state
            .global_semantics
            .values
            .sync_object_prototype_expression(name, Some(prototype));
    }

    pub(in crate::backend::direct_wasm) fn global_object_prototype_expression(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.state
            .global_semantics
            .values
            .object_prototype_expression(name)
    }

    pub(in crate::backend::direct_wasm) fn record_runtime_prototype_variant(
        &mut self,
        name: &str,
        prototype: Option<Expression>,
    ) {
        self.state
            .global_semantics
            .values
            .record_runtime_prototype_variant(name, prototype);
    }
}
