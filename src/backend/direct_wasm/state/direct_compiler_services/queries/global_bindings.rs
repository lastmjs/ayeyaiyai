use super::*;
use crate::backend::direct_wasm::expand_static_call_arguments;
use crate::ir::hir::CallArgument;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn resolve_global_identifier_expression(
        &self,
        name: &str,
    ) -> Option<Expression> {
        self.state.resolve_global_identifier_expression(name)
    }

    pub(in crate::backend::direct_wasm) fn find_global_identifier_binding_name(
        &self,
        identifier: &str,
    ) -> Option<String> {
        self.state.find_global_identifier_binding_name(identifier)
    }

    pub(in crate::backend::direct_wasm) fn find_global_home_object_binding_name(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.state
            .find_global_home_object_binding_name(function_name)
    }

    pub(in crate::backend::direct_wasm) fn find_global_user_function_binding_name(
        &self,
        function_name: &str,
    ) -> Option<String> {
        self.state
            .find_global_user_function_binding_name(function_name)
    }

    pub(in crate::backend::direct_wasm) fn global_binding_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        self.state.global_binding_kind(name)
    }

    pub(in crate::backend::direct_wasm) fn global_has_binding(&self, name: &str) -> bool {
        self.state.global_has_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_has_implicit_binding(&self, name: &str) -> bool {
        self.state.global_has_implicit_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_has_lexical_binding(&self, name: &str) -> bool {
        self.state.global_has_lexical_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_value_binding(
        &self,
        name: &str,
    ) -> Option<&Expression> {
        self.state.global_value_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.state.global_object_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_array_binding(
        &self,
        name: &str,
    ) -> Option<&ArrayValueBinding> {
        self.state.global_array_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_arguments_binding(
        &self,
        name: &str,
    ) -> Option<&ArgumentsValueBinding> {
        self.state.global_arguments_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_array_binding_entries(
        &self,
    ) -> Vec<(String, ArrayValueBinding)> {
        self.state.global_array_binding_entries()
    }

    pub(in crate::backend::direct_wasm) fn expanded_global_static_call_arguments(
        &self,
        arguments: &[CallArgument],
    ) -> Vec<Expression> {
        let global_array_bindings = self
            .global_array_binding_entries()
            .into_iter()
            .collect::<HashMap<_, _>>();
        expand_static_call_arguments(arguments, &global_array_bindings)
    }

    pub(in crate::backend::direct_wasm) fn global_has_prototype_object_binding(
        &self,
        name: &str,
    ) -> bool {
        self.state.global_has_prototype_object_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_prototype_object_binding(
        &self,
        name: &str,
    ) -> Option<&ObjectValueBinding> {
        self.state.global_prototype_object_binding(name)
    }

    pub(in crate::backend::direct_wasm) fn global_function_binding(
        &self,
        name: &str,
    ) -> Option<&LocalFunctionBinding> {
        self.state.global_function_binding(name)
    }
}
