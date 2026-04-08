use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn expression_inherits_from_prototype_for_instanceof(
        &self,
        left: &Expression,
        prototype: &Expression,
    ) -> bool {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(prototype)
            .filter(|resolved| !static_expression_matches(resolved, prototype))
        {
            return self.expression_inherits_from_prototype_for_instanceof(left, &resolved);
        }
        let materialized_prototype = self.materialize_static_expression(prototype);
        if !static_expression_matches(&materialized_prototype, prototype) {
            return self
                .expression_inherits_from_prototype_for_instanceof(left, &materialized_prototype);
        }
        let Expression::Member { object, property } = &materialized_prototype else {
            return false;
        };
        if !matches!(property.as_ref(), Expression::String(name) if name == "prototype") {
            return false;
        }
        let Expression::Identifier(constructor_name) = object.as_ref() else {
            return false;
        };
        match constructor_name.as_str() {
            "Array" => self.expression_is_known_array_value(left),
            "Function" | "AsyncFunction" | "GeneratorFunction" | "AsyncGeneratorFunction" => {
                self.expression_is_known_function_value_for_instanceof(left)
            }
            "Object" => self.expression_is_known_object_like_value_for_instanceof(left),
            "Promise" => self.expression_is_known_promise_instance_for_instanceof(left),
            "WeakRef" => {
                self.expression_is_known_constructor_instance_for_instanceof(left, "WeakRef")
            }
            "Error" => self.expression_is_known_native_error_instance_for_instanceof(left, "Error"),
            name if native_error_runtime_value(name).is_some() => {
                self.expression_is_known_native_error_instance_for_instanceof(left, name)
            }
            name => self.expression_is_known_constructor_instance_for_instanceof(left, name),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_instanceof_prototype_expression(
        &self,
        right: &Expression,
    ) -> Option<Expression> {
        let prototype_property = Expression::String("prototype".to_string());
        if let Some(binding) = self.resolve_member_getter_binding(right, &prototype_property) {
            return self.resolve_function_binding_static_return_expression_with_call_frame(
                &binding,
                &[],
                right,
            );
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(right)
            && let Some(value) =
                object_binding_lookup_value(&object_binding, &prototype_property).cloned()
        {
            return Some(value);
        }
        let materialized_right = self.materialize_static_expression(right);
        if !static_expression_matches(&materialized_right, right) {
            return self.resolve_instanceof_prototype_expression(&materialized_right);
        }
        if matches!(
            self.resolve_function_binding_from_expression(&materialized_right),
            Some(_)
        ) || matches!(
            &materialized_right,
            Expression::Identifier(name) if infer_call_result_kind(name).is_some()
        ) {
            return Some(Expression::Member {
                object: Box::new(materialized_right),
                property: Box::new(prototype_property),
            });
        }
        None
    }
}
