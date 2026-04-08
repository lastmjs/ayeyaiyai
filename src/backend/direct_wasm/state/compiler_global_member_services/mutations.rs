use super::super::{
    CompilerState, GlobalMemberAccessorMutationAccess, GlobalMemberBindingClearAccess,
    GlobalMemberCaptureMutationAccess, GlobalMemberFunctionMutationAccess, LocalFunctionBinding,
    MemberFunctionBindingKey,
};
use std::collections::BTreeMap;

impl GlobalMemberBindingClearAccess for CompilerState {
    fn clear_global_member_bindings_for_name(&mut self, name: &str) {
        self.global_semantics
            .global_members_mut()
            .clear_bindings_for_name(name, true);
    }
}

impl GlobalMemberFunctionMutationAccess for CompilerState {
    fn set_global_member_function_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.global_semantics
            .global_members_mut()
            .set_function_binding(key, binding);
    }

    fn clear_global_member_function_binding(&mut self, key: &MemberFunctionBindingKey) {
        self.global_semantics
            .global_members_mut()
            .clear_function_binding(key);
    }
}

impl GlobalMemberAccessorMutationAccess for CompilerState {
    fn set_global_member_getter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.global_semantics
            .global_members_mut()
            .set_getter_binding(key, binding);
    }

    fn clear_global_member_getter_binding(&mut self, key: &MemberFunctionBindingKey) {
        self.global_semantics
            .global_members_mut()
            .clear_getter_binding(key);
    }

    fn set_global_member_setter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    ) {
        self.global_semantics
            .global_members_mut()
            .set_setter_binding(key, binding);
    }

    fn clear_global_member_setter_binding(&mut self, key: &MemberFunctionBindingKey) {
        self.global_semantics
            .global_members_mut()
            .clear_setter_binding(key);
    }
}

impl GlobalMemberCaptureMutationAccess for CompilerState {
    fn set_global_member_function_capture_slots(
        &mut self,
        key: MemberFunctionBindingKey,
        capture_slots: BTreeMap<String, String>,
    ) {
        self.global_semantics
            .global_members_mut()
            .set_function_capture_slots(key, capture_slots);
    }
}
