use super::*;

impl<'a> GlobalRuntimePrototypeQueryAccess for FunctionCompilerBackend<'a> {
    fn runtime_prototype_binding_count(&self) -> u32 {
        self.global_semantics
            .values
            .runtime_prototype_binding_count()
    }

    fn global_runtime_prototype_binding(
        &self,
        name: &str,
    ) -> Option<&GlobalObjectRuntimePrototypeBinding> {
        self.global_semantics.values.runtime_prototype_binding(name)
    }
}

impl<'a> GlobalIdentifierValueQueryAccess for FunctionCompilerBackend<'a> {
    fn resolve_global_identifier_expression(&self, name: &str) -> Option<Expression> {
        self.global_semantics
            .values
            .resolve_identifier_expression(name)
    }

    fn find_global_identifier_binding_name(&self, identifier: &str) -> Option<String> {
        self.global_semantics
            .values
            .find_identifier_binding_name(identifier)
    }

    fn find_global_home_object_binding_name(&self, function_name: &str) -> Option<String> {
        self.global_semantics
            .values
            .find_home_object_binding_name(function_name)
    }
}

impl<'a> GlobalValueBindingQueryAccess for FunctionCompilerBackend<'a> {
    fn global_value_binding(&self, name: &str) -> Option<&Expression> {
        self.global_semantics.values.value_binding(name)
    }
}

impl<'a> GlobalObjectValueQueryAccess for FunctionCompilerBackend<'a> {
    fn global_object_binding(&self, name: &str) -> Option<&ObjectValueBinding> {
        self.global_semantics.values.object_binding(name)
    }

    fn global_prototype_object_binding(&self, name: &str) -> Option<&ObjectValueBinding> {
        self.global_semantics.values.prototype_object_binding(name)
    }

    fn global_has_prototype_object_binding(&self, name: &str) -> bool {
        self.global_semantics
            .values
            .has_prototype_object_binding(name)
    }

    fn global_proxy_binding(&self, name: &str) -> Option<&ProxyValueBinding> {
        self.global_semantics.values.proxy_binding(name)
    }

    fn global_object_prototype_expression(&self, name: &str) -> Option<&Expression> {
        self.global_semantics
            .values
            .object_prototype_expression(name)
    }
}

impl<'a> GlobalArrayValueQueryAccess for FunctionCompilerBackend<'a> {
    fn global_array_binding(&self, name: &str) -> Option<&ArrayValueBinding> {
        self.global_semantics.values.array_binding(name)
    }

    fn global_array_binding_entries(&self) -> Vec<(String, ArrayValueBinding)> {
        self.global_semantics
            .values
            .array_bindings()
            .iter()
            .map(|(name, binding)| (name.clone(), binding.clone()))
            .collect()
    }

    fn global_array_uses_runtime_state(&self, name: &str) -> bool {
        self.global_semantics.values.array_uses_runtime_state(name)
    }
}

impl<'a> GlobalArgumentsValueQueryAccess for FunctionCompilerBackend<'a> {
    fn global_arguments_binding(&self, name: &str) -> Option<&ArgumentsValueBinding> {
        self.global_semantics.values.arguments_binding(name)
    }
}

impl<'a> GlobalPropertyDescriptorQueryAccess for FunctionCompilerBackend<'a> {
    fn global_property_descriptor(&self, name: &str) -> Option<&GlobalPropertyDescriptorState> {
        self.global_semantics.values.property_descriptor(name)
    }
}
