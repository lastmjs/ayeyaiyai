use super::*;

pub(in crate::backend::direct_wasm) trait GlobalIdentifierValueQueryAccess {
    fn resolve_global_identifier_expression(&self, name: &str) -> Option<Expression>;
    fn find_global_identifier_binding_name(&self, identifier: &str) -> Option<String>;
    fn find_global_home_object_binding_name(&self, function_name: &str) -> Option<String>;
}

pub(in crate::backend::direct_wasm) trait GlobalValueBindingQueryAccess {
    fn global_value_binding(&self, name: &str) -> Option<&Expression>;
}

pub(in crate::backend::direct_wasm) trait GlobalObjectValueQueryAccess {
    fn global_object_binding(&self, name: &str) -> Option<&ObjectValueBinding>;
    fn global_prototype_object_binding(&self, name: &str) -> Option<&ObjectValueBinding>;
    fn global_has_prototype_object_binding(&self, name: &str) -> bool;
    fn global_proxy_binding(&self, name: &str) -> Option<&ProxyValueBinding>;
    fn global_object_prototype_expression(&self, name: &str) -> Option<&Expression>;
}

pub(in crate::backend::direct_wasm) trait GlobalArrayValueQueryAccess {
    fn global_array_binding(&self, name: &str) -> Option<&ArrayValueBinding>;
    fn global_array_binding_entries(&self) -> Vec<(String, ArrayValueBinding)>;
    fn global_array_uses_runtime_state(&self, name: &str) -> bool;
}

pub(in crate::backend::direct_wasm) trait GlobalRuntimePrototypeQueryAccess {
    fn runtime_prototype_binding_count(&self) -> u32;
    fn global_runtime_prototype_binding(
        &self,
        name: &str,
    ) -> Option<&GlobalObjectRuntimePrototypeBinding>;
}

pub(in crate::backend::direct_wasm) trait GlobalArgumentsValueQueryAccess {
    fn global_arguments_binding(&self, name: &str) -> Option<&ArgumentsValueBinding>;
}

pub(in crate::backend::direct_wasm) trait GlobalPropertyDescriptorQueryAccess {
    fn global_property_descriptor(&self, name: &str) -> Option<&GlobalPropertyDescriptorState>;
}
