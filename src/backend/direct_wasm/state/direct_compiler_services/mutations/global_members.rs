use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn clear_owned_global_member_bindings_for_name(
        &mut self,
        name: &str,
    ) {
        GlobalMemberBindingClearAccess::clear_global_member_bindings_for_name(
            &mut self.state,
            name,
        );
    }

    pub(in crate::backend::direct_wasm) fn set_global_member_function_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        GlobalMemberFunctionMutationAccess::set_global_member_function_binding(
            &mut self.state,
            key,
            binding,
        );
    }

    pub(in crate::backend::direct_wasm) fn clear_global_member_function_binding(
        &mut self,
        key: &MemberFunctionBindingKey,
    ) {
        GlobalMemberFunctionMutationAccess::clear_global_member_function_binding(
            &mut self.state,
            key,
        );
    }

    pub(in crate::backend::direct_wasm) fn set_global_member_getter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        GlobalMemberAccessorMutationAccess::set_global_member_getter_binding(
            &mut self.state,
            key,
            binding,
        );
    }

    pub(in crate::backend::direct_wasm) fn clear_global_member_getter_binding(
        &mut self,
        key: &MemberFunctionBindingKey,
    ) {
        GlobalMemberAccessorMutationAccess::clear_global_member_getter_binding(
            &mut self.state,
            key,
        );
    }

    pub(in crate::backend::direct_wasm) fn set_global_member_setter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        GlobalMemberAccessorMutationAccess::set_global_member_setter_binding(
            &mut self.state,
            key,
            binding,
        );
    }

    pub(in crate::backend::direct_wasm) fn clear_global_member_setter_binding(
        &mut self,
        key: &MemberFunctionBindingKey,
    ) {
        GlobalMemberAccessorMutationAccess::clear_global_member_setter_binding(
            &mut self.state,
            key,
        );
    }

    pub(in crate::backend::direct_wasm) fn set_global_member_function_capture_slots(
        &mut self,
        key: MemberFunctionBindingKey,
        capture_slots: BTreeMap<String, String>,
    ) {
        GlobalMemberCaptureMutationAccess::set_global_member_function_capture_slots(
            &mut self.state,
            key,
            capture_slots,
        );
    }
}
