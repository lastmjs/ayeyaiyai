use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn ordinary_to_primitive_member_requires_runtime_with_context(
        &self,
        expression: &Expression,
        method_name: &str,
        current_function_name: Option<&str>,
    ) -> bool {
        let property = Expression::String(method_name.to_string());
        if let Some(getter_binding) = self.resolve_member_getter_binding(expression, &property) {
            return self
                .resolve_static_function_outcome_from_binding_with_context(
                    &getter_binding,
                    &[],
                    current_function_name,
                )
                .is_none();
        }
        if let Some(function_binding) = self.resolve_member_function_binding(expression, &property)
        {
            return self
                .resolve_static_function_outcome_from_binding_with_context(
                    &function_binding,
                    &[],
                    current_function_name,
                )
                .is_none();
        }
        let Some(object_binding) = self.resolve_object_binding_from_expression(expression) else {
            return false;
        };
        let Some(method_value) = object_binding_lookup_value(&object_binding, &property) else {
            return false;
        };
        if self
            .resolve_static_primitive_expression_with_context(method_value, current_function_name)
            .is_some()
        {
            return false;
        }
        self.resolve_function_binding_from_expression_with_context(
            method_value,
            current_function_name,
        )
        .is_some_and(|binding| {
            self.resolve_static_function_outcome_from_binding_with_context(
                &binding,
                &[],
                current_function_name,
            )
            .is_none()
        })
    }

    pub(in crate::backend::direct_wasm) fn ordinary_to_primitive_requires_runtime_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> bool {
        if matches!(expression, Expression::Call { .. } | Expression::New { .. })
            && self
                .resolve_object_binding_from_expression(expression)
                .is_some()
        {
            let (callee, arguments) = match expression {
                Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                    (callee.as_ref(), arguments.as_slice())
                }
                _ => unreachable!("filtered above"),
            };
            if self
                .resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    current_function_name,
                )
                .is_none()
            {
                return true;
            }
        }

        ["valueOf", "toString"].into_iter().any(|method_name| {
            self.ordinary_to_primitive_member_requires_runtime_with_context(
                expression,
                method_name,
                current_function_name,
            )
        })
    }

    pub(in crate::backend::direct_wasm) fn symbol_to_primitive_requires_runtime_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> bool {
        let symbol_property = symbol_to_primitive_expression();
        let default_argument = [CallArgument::Expression(Expression::String(
            "default".to_string(),
        ))];

        if let Some(getter_binding) =
            self.resolve_member_getter_binding(expression, &symbol_property)
        {
            let Some(getter_outcome) = self
                .resolve_static_function_outcome_from_binding_with_context(
                    &getter_binding,
                    &[],
                    current_function_name,
                )
            else {
                return true;
            };
            let method_value = match getter_outcome {
                StaticEvalOutcome::Throw(_) => return false,
                StaticEvalOutcome::Value(method_value) => method_value,
            };
            if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                &method_value,
                current_function_name,
            ) {
                return !matches!(primitive, Expression::Null | Expression::Undefined)
                    && self
                        .resolve_function_binding_from_expression_with_context(
                            &primitive,
                            current_function_name,
                        )
                        .is_some();
            }
            let Some(binding) = self.resolve_function_binding_from_expression_with_context(
                &method_value,
                current_function_name,
            ) else {
                return false;
            };
            return self
                .resolve_static_function_outcome_from_binding_with_context(
                    &binding,
                    &default_argument,
                    current_function_name,
                )
                .is_none();
        }

        if let Some(function_binding) =
            self.resolve_member_function_binding(expression, &symbol_property)
        {
            return self
                .resolve_static_function_outcome_from_binding_with_context(
                    &function_binding,
                    &default_argument,
                    current_function_name,
                )
                .is_none();
        }

        let Some(object_binding) = self.resolve_object_binding_from_expression(expression) else {
            return false;
        };
        let Some(method_value) = object_binding_lookup_value(&object_binding, &symbol_property)
        else {
            return false;
        };
        if let Some(primitive) = self
            .resolve_static_primitive_expression_with_context(method_value, current_function_name)
        {
            return !matches!(primitive, Expression::Null | Expression::Undefined)
                && self
                    .resolve_function_binding_from_expression_with_context(
                        &primitive,
                        current_function_name,
                    )
                    .is_some();
        }
        let Some(binding) = self.resolve_function_binding_from_expression_with_context(
            method_value,
            current_function_name,
        ) else {
            return false;
        };
        self.resolve_static_function_outcome_from_binding_with_context(
            &binding,
            &default_argument,
            current_function_name,
        )
        .is_none()
    }

    pub(in crate::backend::direct_wasm) fn addition_operand_requires_runtime_value(
        &self,
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Identifier(name) => {
                !matches!(name.as_str(), "undefined" | "NaN" | "Infinity")
            }
            Expression::Member { .. }
            | Expression::SuperMember { .. }
            | Expression::Assign { .. }
            | Expression::AssignMember { .. }
            | Expression::AssignSuperMember { .. }
            | Expression::Call { .. }
            | Expression::SuperCall { .. }
            | Expression::New { .. }
            | Expression::This
            | Expression::Await(_)
            | Expression::EnumerateKeys(_)
            | Expression::GetIterator(_)
            | Expression::IteratorClose(_)
            | Expression::Update { .. }
            | Expression::NewTarget
            | Expression::Sent => true,
            Expression::Unary { expression, .. } => {
                self.addition_operand_requires_runtime_value(expression)
            }
            Expression::Binary { left, right, .. } => {
                self.addition_operand_requires_runtime_value(left)
                    || self.addition_operand_requires_runtime_value(right)
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.addition_operand_requires_runtime_value(condition)
                    || self.addition_operand_requires_runtime_value(then_expression)
                    || self.addition_operand_requires_runtime_value(else_expression)
            }
            Expression::Sequence(expressions) => expressions
                .iter()
                .any(|expression| self.addition_operand_requires_runtime_value(expression)),
            Expression::Array(elements) => elements.iter().any(|element| match element {
                ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                    self.addition_operand_requires_runtime_value(expression)
                }
            }),
            Expression::Object(entries) => entries.iter().any(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    self.addition_operand_requires_runtime_value(key)
                        || self.addition_operand_requires_runtime_value(value)
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                    self.addition_operand_requires_runtime_value(key)
                        || self.addition_operand_requires_runtime_value(getter)
                }
                crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                    self.addition_operand_requires_runtime_value(key)
                        || self.addition_operand_requires_runtime_value(setter)
                }
                crate::ir::hir::ObjectEntry::Spread(expression) => {
                    self.addition_operand_requires_runtime_value(expression)
                }
            }),
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined => false,
        }
    }
}
