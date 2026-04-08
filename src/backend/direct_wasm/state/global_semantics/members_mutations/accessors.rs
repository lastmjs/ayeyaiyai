use super::super::super::*;

impl GlobalMemberService {
    pub(in crate::backend::direct_wasm) fn set_getter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.member_getter_bindings.insert(key, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_getter_binding(
        &mut self,
        key: &MemberFunctionBindingKey,
    ) {
        self.member_getter_bindings.remove(key);
    }

    pub(in crate::backend::direct_wasm) fn set_setter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.member_setter_bindings.insert(key, binding);
    }

    pub(in crate::backend::direct_wasm) fn clear_setter_binding(
        &mut self,
        key: &MemberFunctionBindingKey,
    ) {
        self.member_setter_bindings.remove(key);
    }
}
