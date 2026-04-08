use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn is_named_global_array_binding(
        &self,
        name: &str,
    ) -> bool {
        self.resolve_current_local_binding(name).is_none()
            && self.backend.global_array_binding(name).is_some()
    }

    pub(in crate::backend::direct_wasm) fn uses_global_runtime_array_state(
        &self,
        name: &str,
    ) -> bool {
        self.backend.global_array_uses_runtime_state(name)
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_length_binding_name(
        &self,
        name: &str,
    ) -> String {
        format!("__ayy_global_array_length_{name}")
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_slot_binding_name(
        &self,
        name: &str,
        index: u32,
    ) -> String {
        format!("__ayy_global_array_slot_{name}_{index}")
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_length_binding(
        &mut self,
        name: &str,
    ) -> ImplicitGlobalBinding {
        let hidden_name = self.global_runtime_array_length_binding_name(name);
        self.ensure_implicit_global_binding(&hidden_name)
    }

    pub(in crate::backend::direct_wasm) fn global_runtime_array_slot_binding(
        &mut self,
        name: &str,
        index: u32,
    ) -> ImplicitGlobalBinding {
        let hidden_name = self.global_runtime_array_slot_binding_name(name, index);
        self.ensure_implicit_global_binding(&hidden_name)
    }
}
