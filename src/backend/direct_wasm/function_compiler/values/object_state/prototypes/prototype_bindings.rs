use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn merge_object_binding_properties(
        target: &mut ObjectValueBinding,
        source: &ObjectValueBinding,
    ) {
        for (name, value) in &source.string_properties {
            let enumerable = !source
                .non_enumerable_string_properties
                .iter()
                .any(|hidden_name| hidden_name == name);
            object_binding_define_property(
                target,
                Expression::String(name.clone()),
                value.clone(),
                enumerable,
            );
        }
        for (property, value) in &source.symbol_properties {
            object_binding_define_property(target, property.clone(), value.clone(), true);
        }
    }

    pub(in crate::backend::direct_wasm) fn default_function_prototype_object_binding(
        &self,
        function_binding: &LocalFunctionBinding,
    ) -> Option<ObjectValueBinding> {
        let constructor_expression = match function_binding {
            LocalFunctionBinding::User(function_name) => {
                let user_function = self.user_function(function_name)?;
                if !user_function.is_constructible() {
                    return None;
                }
                Expression::Identifier(function_name.clone())
            }
            LocalFunctionBinding::Builtin(function_name) => {
                if !is_function_constructor_builtin(function_name) {
                    return None;
                }
                Expression::Identifier(function_name.clone())
            }
        };

        let mut object_binding = empty_object_value_binding();
        object_binding_define_property(
            &mut object_binding,
            Expression::String("constructor".to_string()),
            constructor_expression,
            false,
        );
        Some(object_binding)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_prototype_object_binding(
        &self,
        name: &str,
    ) -> Option<ObjectValueBinding> {
        let stored_binding = self
            .state
            .speculation
            .static_semantics
            .objects
            .local_prototype_object_bindings
            .get(name)
            .cloned()
            .or_else(|| {
                self.backend
                    .global_semantics
                    .values
                    .prototype_object_bindings
                    .get(name)
                    .cloned()
            });
        let default_binding = self
            .resolve_function_binding_from_expression(&Expression::Identifier(name.to_string()))
            .and_then(|binding| self.default_function_prototype_object_binding(&binding));

        match (default_binding, stored_binding) {
            (Some(mut default_binding), Some(stored_binding)) => {
                Self::merge_object_binding_properties(&mut default_binding, &stored_binding);
                Some(default_binding)
            }
            (Some(default_binding), None) => Some(default_binding),
            (None, Some(stored_binding)) => Some(stored_binding),
            (None, None) => None,
        }
    }
}
