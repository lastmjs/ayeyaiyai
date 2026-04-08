use crate::backend::direct_wasm::{LocalFunctionBinding, MemberFunctionBindingKey};
use std::collections::BTreeMap;

pub(in crate::backend::direct_wasm) trait GlobalMemberFunctionQueryAccess {
    fn global_member_function_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)>;
    fn global_member_function_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding>;
}

pub(in crate::backend::direct_wasm) trait GlobalMemberCaptureQueryAccess {
    fn global_member_function_capture_slots(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&BTreeMap<String, String>>;
    fn global_member_function_capture_slot_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, BTreeMap<String, String>)>;
}

pub(in crate::backend::direct_wasm) trait GlobalMemberAccessorQueryAccess {
    fn global_member_getter_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)>;
    fn global_member_getter_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding>;
    fn global_member_setter_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)>;
    fn global_member_setter_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding>;
}

pub(in crate::backend::direct_wasm) trait GlobalMemberBindingClearAccess {
    fn clear_global_member_bindings_for_name(&mut self, name: &str);
}

pub(in crate::backend::direct_wasm) trait GlobalMemberFunctionMutationAccess {
    fn set_global_member_function_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    );
    fn clear_global_member_function_binding(&mut self, key: &MemberFunctionBindingKey);
}

pub(in crate::backend::direct_wasm) trait GlobalMemberAccessorMutationAccess {
    fn set_global_member_getter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    );
    fn clear_global_member_getter_binding(&mut self, key: &MemberFunctionBindingKey);
    fn set_global_member_setter_binding(
        &mut self,
        key: MemberFunctionBindingKey,
        binding: LocalFunctionBinding,
    );
    fn clear_global_member_setter_binding(&mut self, key: &MemberFunctionBindingKey);
}

pub(in crate::backend::direct_wasm) trait GlobalMemberCaptureMutationAccess {
    fn set_global_member_function_capture_slots(
        &mut self,
        key: MemberFunctionBindingKey,
        capture_slots: BTreeMap<String, String>,
    );
}
