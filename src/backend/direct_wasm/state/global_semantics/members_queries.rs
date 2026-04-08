use super::super::*;

impl GlobalMemberService {
    pub(in crate::backend::direct_wasm) fn function_bindings(
        &self,
    ) -> &HashMap<MemberFunctionBindingKey, LocalFunctionBinding> {
        &self.member_function_bindings
    }

    pub(in crate::backend::direct_wasm) fn function_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.member_function_bindings.get(key)
    }

    pub(in crate::backend::direct_wasm) fn function_capture_slots(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&BTreeMap<String, String>> {
        self.member_function_capture_slots.get(key)
    }

    pub(in crate::backend::direct_wasm) fn function_capture_slots_map(
        &self,
    ) -> &HashMap<MemberFunctionBindingKey, BTreeMap<String, String>> {
        &self.member_function_capture_slots
    }

    pub(in crate::backend::direct_wasm) fn getter_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.member_getter_bindings.get(key)
    }

    pub(in crate::backend::direct_wasm) fn getter_bindings(
        &self,
    ) -> &HashMap<MemberFunctionBindingKey, LocalFunctionBinding> {
        &self.member_getter_bindings
    }

    pub(in crate::backend::direct_wasm) fn setter_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.member_setter_bindings.get(key)
    }

    pub(in crate::backend::direct_wasm) fn setter_bindings(
        &self,
    ) -> &HashMap<MemberFunctionBindingKey, LocalFunctionBinding> {
        &self.member_setter_bindings
    }
}
