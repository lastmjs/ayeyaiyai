use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn global_member_function_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.state.global_member_function_binding(key)
    }

    pub(in crate::backend::direct_wasm) fn global_member_getter_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.state.global_member_getter_binding(key)
    }

    pub(in crate::backend::direct_wasm) fn has_global_member_function_capture_slots(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> bool {
        self.global_member_function_capture_slots(key).is_some()
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_capture_slots(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&BTreeMap<String, String>> {
        self.state.global_member_function_capture_slots(key)
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)> {
        self.state.global_member_function_binding_entries()
    }

    pub(in crate::backend::direct_wasm) fn global_member_getter_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)> {
        self.state.global_member_getter_binding_entries()
    }

    pub(in crate::backend::direct_wasm) fn global_member_setter_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)> {
        self.state.global_member_setter_binding_entries()
    }

    pub(in crate::backend::direct_wasm) fn global_member_function_capture_slot_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, BTreeMap<String, String>)> {
        self.state.global_member_function_capture_slot_entries()
    }
}
