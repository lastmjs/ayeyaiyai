use super::guard::FunctionBindingResolutionGuard;
use super::*;

impl<'a> FunctionCompiler<'a> {
    fn resolve_bound_builtin_function_binding_from_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        let Expression::Member { object, property } = callee else {
            return None;
        };
        if !matches!(property.as_ref(), Expression::String(name) if name == "bind") {
            return None;
        }
        let LocalFunctionBinding::Builtin(function_name) = self
            .resolve_function_binding_from_expression_with_context(object, current_function_name)?
        else {
            return None;
        };
        if function_name != "Function.prototype.call" {
            return None;
        }
        let [
            CallArgument::Expression(target) | CallArgument::Spread(target),
            ..,
        ] = arguments
        else {
            return None;
        };
        let LocalFunctionBinding::Builtin(target_name) = self
            .resolve_function_binding_from_expression_with_context(target, current_function_name)?
        else {
            return None;
        };
        Some(LocalFunctionBinding::Builtin(
            bound_function_prototype_call_builtin_name(&target_name),
        ))
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_function_binding_from_expression_with_context(
            expression,
            self.current_function_name(),
        )
    }

    pub(in crate::backend::direct_wasm) fn is_restricted_arrow_function_property(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> bool {
        matches!(
            property,
            Expression::String(property_name)
                if property_name == "caller" || property_name == "arguments"
        ) && self
            .resolve_user_function_from_expression(object)
            .is_some_and(UserFunction::is_arrow)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_expression_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        let _guard = FunctionBindingResolutionGuard::enter(expression, current_function_name);
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            if let Some(binding) = self.resolve_function_binding_from_expression_with_context(
                &resolved,
                current_function_name,
            ) {
                return Some(binding);
            }
        }
        let binding = match expression {
            Expression::Identifier(name) => {
                if let Some((resolved_name, _)) = self.resolve_current_local_binding(name) {
                    self.state
                        .speculation
                        .static_semantics
                        .local_function_binding(&resolved_name)
                        .cloned()
                } else if self.resolve_eval_local_function_hidden_name(name).is_some() {
                    self.state
                        .speculation
                        .static_semantics
                        .local_function_binding(name)
                        .cloned()
                } else if builtin_function_runtime_value(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else if let Some(function_binding) = self
                    .backend
                    .global_semantics
                    .functions
                    .function_binding(name)
                {
                    Some(function_binding.clone())
                } else if is_internal_user_function_identifier(name)
                    && self.contains_user_function(name)
                {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if name == "eval" || self.infer_call_result_kind(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            Expression::Assign { value, .. } => self
                .resolve_function_binding_from_expression_with_context(
                    value,
                    current_function_name,
                ),
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                if let Expression::Call { .. } = expression
                    && let Some(binding) = self.resolve_bound_builtin_function_binding_from_call(
                        callee,
                        arguments,
                        current_function_name,
                    )
                {
                    return Some(binding);
                }
                self.resolve_static_call_result_expression_with_context(
                    callee,
                    arguments,
                    current_function_name,
                )
                .and_then(|(value, callee_function_name)| {
                    self.resolve_function_binding_from_expression_with_context(
                        &value,
                        callee_function_name.as_deref().or(current_function_name),
                    )
                })
                .or_else(|| self.resolve_returned_function_binding_from_call(callee, arguments))
            }
            Expression::Binary { op, left, right }
                if matches!(
                    op,
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr | BinaryOp::NullishCoalescing
                ) =>
            {
                self.resolve_static_logical_result_expression(*op, left, right)
                    .and_then(|resolved| {
                        self.resolve_function_binding_from_expression_with_context(
                            &resolved,
                            current_function_name,
                        )
                    })
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                if let Some(condition_value) = self.resolve_static_if_condition_value(condition) {
                    let branch = if condition_value {
                        then_expression
                    } else {
                        else_expression
                    };
                    self.resolve_function_binding_from_expression_with_context(
                        branch,
                        current_function_name,
                    )
                } else {
                    let then_binding = self.resolve_function_binding_from_expression_with_context(
                        then_expression,
                        current_function_name,
                    );
                    let else_binding = self.resolve_function_binding_from_expression_with_context(
                        else_expression,
                        current_function_name,
                    );
                    match (then_binding, else_binding) {
                        (Some(then_binding), Some(else_binding))
                            if then_binding == else_binding =>
                        {
                            Some(then_binding)
                        }
                        _ => None,
                    }
                }
            }
            Expression::Sequence(expressions) => expressions.last().and_then(|expression| {
                self.resolve_function_binding_from_expression_with_context(
                    expression,
                    current_function_name,
                )
            }),
            Expression::Member { object, property } => {
                if matches!(property.as_ref(), Expression::String(name) if name == "constructor")
                    && self
                        .resolve_function_binding_from_expression(object)
                        .is_some()
                {
                    return Some(LocalFunctionBinding::Builtin(
                        FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN.to_string(),
                    ));
                }
                if matches!(property.as_ref(), Expression::String(name) if name == "value") {
                    if let Some(IteratorStepBinding::Runtime {
                        function_binding: Some(function_binding),
                        ..
                    }) = self.resolve_iterator_step_binding_from_expression(object)
                    {
                        return Some(function_binding);
                    }
                }
                if let Some(value) =
                    self.resolve_returned_member_value_from_expression(object, property)
                {
                    self.resolve_function_binding_from_expression(&value)
                } else if let Some(getter_binding) =
                    self.resolve_member_getter_binding(object, property)
                    && let Some(value) = self
                        .resolve_function_binding_static_return_expression_with_call_frame(
                            &getter_binding,
                            &[],
                            object,
                        )
                {
                    self.resolve_function_binding_from_expression(&value)
                } else {
                    self.resolve_member_function_binding(object, property)
                }
            }
            Expression::SuperMember { property } => {
                self.resolve_super_function_binding_with_context(property, current_function_name)
            }
            _ => None,
        };
        if binding.is_some() {
            return binding;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_function_binding_from_expression_with_context(
                &materialized,
                current_function_name,
            );
        }
        None
    }
}
