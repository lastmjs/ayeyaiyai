use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_iterable_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        if let Some(binding) = self.resolve_static_user_iterator_binding(expression) {
            return Some(binding);
        }
        let object_binding = self.resolve_object_binding_from_expression(expression)?;
        let symbol_iterator = self.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("iterator".to_string())),
        });
        let iterator_method =
            object_binding_lookup_value(&object_binding, &symbol_iterator)?.clone();
        let LocalFunctionBinding::User(iterator_function_name) =
            self.resolve_function_binding_from_expression(&iterator_method)?
        else {
            return None;
        };
        let (iterator_result, iterator_bindings) = self
            .execute_simple_static_user_function_with_bindings(
                &iterator_function_name,
                &HashMap::new(),
            )?;
        let iterator_result_binding =
            self.resolve_object_binding_from_expression(&iterator_result)?;
        let next_value = object_binding_lookup_value(
            &iterator_result_binding,
            &Expression::String("next".to_string()),
        )?
        .clone();
        let LocalFunctionBinding::User(next_function_name) =
            self.resolve_function_binding_from_expression(&next_value)?
        else {
            return None;
        };

        let mut step_bindings = iterator_bindings;
        let mut values = Vec::new();
        for _ in 0..256 {
            let (step_result, updated_bindings) = self
                .execute_simple_static_user_function_with_bindings(
                    &next_function_name,
                    &step_bindings,
                )?;
            step_bindings = updated_bindings;
            let step_object_binding = self.resolve_object_binding_from_expression(&step_result)?;
            let done = object_binding_lookup_value(
                &step_object_binding,
                &Expression::String("done".to_string()),
            )
            .cloned()
            .unwrap_or(Expression::Bool(false));
            let value = object_binding_lookup_value(
                &step_object_binding,
                &Expression::String("value".to_string()),
            )
            .cloned()
            .unwrap_or(Expression::Undefined);
            match done {
                Expression::Bool(true) => return Some(ArrayValueBinding { values }),
                Expression::Bool(false) => values.push(Some(value)),
                _ => return None,
            }
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_user_iterator_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        let (user_function, _) = self.resolve_user_function_call_target(expression)?;
        let next_binding = user_function
            .returned_member_function_bindings
            .iter()
            .find(|binding| binding.property == "next")?;
        let LocalFunctionBinding::User(next_function_name) = &next_binding.binding else {
            return None;
        };
        let mut property_bindings =
            self.resolve_returned_member_capture_bindings_for_value(expression)?;
        let capture_bindings = property_bindings.remove("next")?;

        let mut bindings = capture_bindings;
        let mut values = Vec::new();
        for _ in 0..256 {
            let (step_result, updated_bindings) =
                self.resolve_bound_snapshot_user_function_result(next_function_name, &bindings)?;
            bindings = updated_bindings;
            let step_object_binding = self.resolve_object_binding_from_expression(&step_result)?;
            let done = object_binding_lookup_value(
                &step_object_binding,
                &Expression::String("done".to_string()),
            )
            .cloned()
            .unwrap_or(Expression::Bool(false));
            let value = object_binding_lookup_value(
                &step_object_binding,
                &Expression::String("value".to_string()),
            )
            .cloned()
            .unwrap_or(Expression::Undefined);
            match done {
                Expression::Bool(true) => return Some(ArrayValueBinding { values }),
                Expression::Bool(false) => values.push(Some(value)),
                _ => return None,
            }
        }

        None
    }
}
