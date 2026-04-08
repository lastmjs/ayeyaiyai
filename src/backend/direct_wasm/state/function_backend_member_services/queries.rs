use super::super::{
    FunctionCompilerBackend, GlobalMemberAccessorQueryAccess, GlobalMemberCaptureQueryAccess,
    GlobalMemberFunctionQueryAccess, LocalFunctionBinding, MemberFunctionBindingKey,
};
use std::collections::BTreeMap;

impl<'a> GlobalMemberFunctionQueryAccess for FunctionCompilerBackend<'a> {
    fn global_member_function_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)> {
        self.global_semantics
            .global_members()
            .function_bindings()
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()))
            .collect()
    }

    fn global_member_function_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.global_semantics.global_members().function_binding(key)
    }
}

impl<'a> GlobalMemberCaptureQueryAccess for FunctionCompilerBackend<'a> {
    fn global_member_function_capture_slots(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&BTreeMap<String, String>> {
        self.global_semantics
            .global_members()
            .function_capture_slots(key)
    }

    fn global_member_function_capture_slot_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, BTreeMap<String, String>)> {
        self.global_semantics
            .global_members()
            .function_capture_slots_map()
            .iter()
            .map(|(key, capture_slots)| (key.clone(), capture_slots.clone()))
            .collect()
    }
}

impl<'a> GlobalMemberAccessorQueryAccess for FunctionCompilerBackend<'a> {
    fn global_member_getter_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)> {
        self.global_semantics
            .global_members()
            .getter_bindings()
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()))
            .collect()
    }

    fn global_member_getter_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.global_semantics.global_members().getter_binding(key)
    }

    fn global_member_setter_binding_entries(
        &self,
    ) -> Vec<(MemberFunctionBindingKey, LocalFunctionBinding)> {
        self.global_semantics
            .global_members()
            .setter_bindings()
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()))
            .collect()
    }

    fn global_member_setter_binding(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> Option<&LocalFunctionBinding> {
        self.global_semantics.global_members().setter_binding(key)
    }
}
