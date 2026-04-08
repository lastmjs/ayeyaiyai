use super::super::super::*;

impl GlobalMemberService {
    pub(in crate::backend::direct_wasm) fn set_function_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.member_function_bindings.insert(key, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_function_binding(
        &mut self,
        key: &MemberFunctionBindingKey,
    ) {
        self.member_function_bindings.remove(key);
    }

    pub(in crate::backend::direct_wasm) fn set_function_capture_slots(
        &mut self,
        key: MemberFunctionBindingKey,
        capture_slots: BTreeMap<String, String>,
    ) {
        self.member_function_capture_slots
            .insert(key, capture_slots);
    }
}
