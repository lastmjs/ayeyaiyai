use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn prepare_identifier_value_store(
        &mut self,
        name: &str,
        value_expression: &Expression,
    ) -> PreparedIdentifierValueStore {
        let canonical_value_expression = self
            .prepare_special_assignment_expression(value_expression)
            .unwrap_or_else(|| value_expression.clone());
        let tracked_value_expression = match &canonical_value_expression {
            Expression::Call { callee, arguments } => {
                if self
                    .resolve_user_function_from_expression(callee)
                    .is_some_and(|user_function| user_function.is_async())
                {
                    canonical_value_expression.clone()
                } else {
                    self.resolve_static_call_result_expression_with_context(
                        callee,
                        arguments,
                        self.current_function_name(),
                    )
                    .map(|(value, _)| value)
                    .unwrap_or_else(|| canonical_value_expression.clone())
                }
            }
            Expression::Member { object, property } => {
                if self
                    .resolve_member_function_capture_slots(object, property)
                    .is_some()
                {
                    canonical_value_expression.clone()
                } else {
                    self.resolve_member_getter_binding(object, property)
                        .and_then(|binding| {
                            self.resolve_function_binding_static_return_expression_with_call_frame(
                                &binding,
                                &[],
                                object,
                            )
                        })
                        .unwrap_or_else(|| canonical_value_expression.clone())
                }
            }
            _ => canonical_value_expression.clone(),
        };
        let resolved_descriptor_binding =
            self.resolve_descriptor_binding_from_expression(&canonical_value_expression);
        let returned_descriptor_binding = match &canonical_value_expression {
            Expression::Call { callee, arguments } => self
                .resolve_function_binding_from_expression(callee)
                .and_then(|binding| match binding {
                    LocalFunctionBinding::User(function_name) => self
                        .resolve_static_returned_descriptor_binding_from_user_function_call(
                            &function_name,
                            arguments,
                        ),
                    LocalFunctionBinding::Builtin(_) => None,
                }),
            _ => None,
        };
        let descriptor_binding_expression = if resolved_descriptor_binding.is_some() {
            canonical_value_expression.clone()
        } else {
            tracked_value_expression.clone()
        };
        let tracked_object_expression = resolved_descriptor_binding
            .as_ref()
            .map(|descriptor| {
                object_binding_to_expression(
                    &self.object_binding_from_property_descriptor(descriptor),
                )
            })
            .unwrap_or_else(|| tracked_value_expression.clone());
        let matched_call_snapshot = matches!(
            canonical_value_expression,
            Expression::Call { .. } | Expression::New { .. }
        )
        .then(|| {
            self.state
                .speculation
                .static_semantics
                .last_bound_user_function_call
                .as_ref()
                .and_then(|snapshot| {
                    let source_expression = snapshot.source_expression.as_ref()?;
                    let materialized_source = self.materialize_static_expression(source_expression);
                    let materialized_value =
                        self.materialize_static_expression(&canonical_value_expression);
                    static_expression_matches(&materialized_source, &materialized_value)
                        .then_some(snapshot)
                })
        })
        .flatten();
        let call_result_snapshot_expression = matched_call_snapshot.and_then(|snapshot| {
            snapshot
                .result_expression
                .as_ref()
                .map(|result| self.materialize_static_expression(result))
        });
        let call_source_snapshot_expression =
            matched_call_snapshot.and_then(|snapshot| snapshot.source_expression.as_ref().cloned());
        let function_binding_expression = call_result_snapshot_expression
            .as_ref()
            .filter(|expression| {
                self.resolve_function_binding_from_expression(expression)
                    .is_some()
            })
            .unwrap_or(&tracked_value_expression)
            .clone();
        let function_binding =
            self.resolve_function_binding_from_expression(&function_binding_expression);
        let object_binding_expression = call_result_snapshot_expression
            .as_ref()
            .filter(|expression| {
                self.resolve_object_binding_from_expression(expression)
                    .is_some()
            })
            .unwrap_or(&tracked_object_expression)
            .clone();
        let kind = self.infer_value_kind(&tracked_value_expression);
        let static_string_value = if kind == Some(StaticValueKind::String) {
            self.resolve_static_string_value(&tracked_value_expression)
        } else {
            None
        };
        let exact_static_number = self
            .resolve_static_number_value(&tracked_value_expression)
            .filter(|number| {
                number.is_nan()
                    || !number.is_finite()
                    || number.fract() != 0.0
                    || (*number == 0.0 && number.is_sign_negative())
            });
        let array_binding = self.resolve_array_binding_from_expression(&tracked_value_expression);
        let preserve_tracked_member_expression = matches!(
            &tracked_value_expression,
            Expression::Member { object, property }
                if self
                    .resolve_member_function_capture_slots(object, property)
                    .is_some()
        );
        let module_assignment_expression = if preserve_tracked_member_expression {
            tracked_value_expression.clone()
        } else {
            self.materialize_static_expression(&tracked_value_expression)
        };
        let resolved_local_binding = self.resolve_current_local_binding(name);

        PreparedIdentifierValueStore {
            canonical_value_expression,
            tracked_value_expression,
            descriptor_binding_expression,
            tracked_object_expression,
            call_source_snapshot_expression,
            function_binding_expression,
            function_binding,
            object_binding_expression,
            kind,
            static_string_value,
            exact_static_number,
            array_binding,
            module_assignment_expression,
            resolved_local_binding,
            returned_descriptor_binding,
        }
    }
}
