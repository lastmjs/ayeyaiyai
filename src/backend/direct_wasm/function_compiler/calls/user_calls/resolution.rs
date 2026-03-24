use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_user_function_construct(
        &mut self,
        callee: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !user_function.is_constructible() {
            return Ok(false);
        }

        self.last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
            function_name: user_function.name.clone(),
            source_expression: Some(Expression::New {
                callee: Box::new(callee.clone()),
                arguments: arguments.to_vec(),
            }),
            result_expression: self
                .resolve_user_constructor_object_binding_for_function(user_function, arguments)
                .map(|binding| object_binding_to_expression(&binding)),
            updated_bindings: HashMap::new(),
        });

        self.emit_user_function_call_with_new_target(
            user_function,
            arguments,
            user_function_runtime_value(user_function),
        )?;
        self.instructions.push(0x1a);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_function_binding_from_expression_with_context(
            expression,
            self.current_user_function_name.as_deref(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<&UserFunction> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(expression)?
        else {
            return None;
        };
        self.module.user_function_map.get(&function_name)
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
                    self.local_function_bindings.get(&resolved_name).cloned()
                } else if self.resolve_eval_local_function_hidden_name(name).is_some() {
                    self.local_function_bindings.get(name).cloned()
                } else if builtin_function_runtime_value(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else if let Some(function_binding) =
                    self.module.global_function_bindings.get(name)
                {
                    Some(function_binding.clone())
                } else if is_internal_user_function_identifier(name)
                    && self.module.user_function_map.contains_key(name)
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

    pub(in crate::backend::direct_wasm) fn resolve_test262_realm_id_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(realm_id) = parse_test262_realm_identifier(name) {
                    return Some(realm_id);
                }
                let resolved = self.resolve_bound_alias_expression(expression)?;
                let Expression::Identifier(name) = resolved else {
                    return None;
                };
                parse_test262_realm_identifier(&name)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_test262_realm_global_id_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        let materialized = self.materialize_static_expression(expression);
        match &materialized {
            Expression::Identifier(name) => parse_test262_realm_global_identifier(name),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "global") => {
                self.resolve_test262_realm_id_from_expression(object)
            }
            _ => None,
        }
    }
}
