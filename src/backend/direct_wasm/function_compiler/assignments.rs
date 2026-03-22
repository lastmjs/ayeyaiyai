use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn prepare_special_assignment_expression(
        &mut self,
        expression: &Expression,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => {
                let resolved = self.resolve_bound_alias_expression(expression)?;
                let Expression::Identifier(resolved_name) = resolved else {
                    return None;
                };
                if resolved_name != *name
                    && (parse_test262_realm_identifier(&resolved_name).is_some()
                        || parse_test262_realm_global_identifier(&resolved_name).is_some())
                {
                    return Some(Expression::Identifier(resolved_name));
                }
                None
            }
            Expression::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { object, property }
                            if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                                && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
                    ) =>
            {
                let realm_id = self.module.allocate_test262_realm();
                Some(Expression::Identifier(test262_realm_identifier(realm_id)))
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "global") =>
            {
                let realm_expression = self
                    .prepare_special_assignment_expression(object)
                    .unwrap_or_else(|| self.materialize_static_expression(object));
                let Expression::Identifier(realm_name) = realm_expression else {
                    return None;
                };
                let realm_id = parse_test262_realm_identifier(&realm_name)?;
                Some(Expression::Identifier(test262_realm_global_identifier(
                    realm_id,
                )))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn member_function_binding_key(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<MemberFunctionBindingKey> {
        let target = match object {
            Expression::Identifier(name) => MemberFunctionBindingTarget::Identifier(name.clone()),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "prototype") =>
            {
                let Expression::Identifier(name) = object.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            Expression::New { callee, .. } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                MemberFunctionBindingTarget::Prototype(name.clone())
            }
            _ => return None,
        };

        let property = self.member_function_binding_property(property)?;

        Some(MemberFunctionBindingKey { target, property })
    }

    pub(in crate::backend::direct_wasm) fn member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let resolved_property = self
            .resolve_bound_alias_expression(property)
            .filter(|resolved| !static_expression_matches(resolved, property))
            .unwrap_or_else(|| property.clone());

        for candidate in [property, &resolved_property] {
            if let Some(property_name) = static_property_name_from_expression(candidate) {
                return Some(MemberFunctionBindingProperty::String(property_name));
            }
            if let Some(symbol_name) = self.well_known_symbol_name(candidate) {
                return Some(MemberFunctionBindingProperty::Symbol(symbol_name));
            }
            if let Some(Expression::Identifier(symbol_name)) =
                self.resolve_symbol_identity_expression(candidate)
            {
                return Some(MemberFunctionBindingProperty::Symbol(symbol_name));
            }
        }

        match &resolved_property {
            Expression::Call { callee, .. }
                if matches!(callee.as_ref(), Expression::Identifier(name)
                    if name == "Symbol" && self.is_unshadowed_builtin_identifier(name)) =>
            {
                Some(MemberFunctionBindingProperty::SymbolExpression(format!(
                    "{resolved_property:?}"
                )))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_primitive_property_key_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let materialized = self.materialize_static_expression(expression);
        if let Some(property_name) = static_property_name_from_expression(&materialized) {
            return Some(Expression::String(property_name));
        }
        if self.well_known_symbol_name(&materialized).is_some() {
            return Some(materialized);
        }
        self.resolve_symbol_identity_expression(&materialized)
    }

    pub(in crate::backend::direct_wasm) fn well_known_symbol_name(
        &self,
        expression: &Expression,
    ) -> Option<String> {
        let Expression::Member { object, property } = expression else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol" && self.is_unshadowed_builtin_identifier(name))
        {
            return None;
        }
        let Expression::String(name) = property.as_ref() else {
            return None;
        };
        Some(format!("Symbol.{name}"))
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key_from_function_binding(
        &self,
        binding: &LocalFunctionBinding,
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.module.user_function_map.get(function_name)?;
        let summary = user_function.inline_summary.as_ref()?;
        let return_value = summary.return_value.as_ref()?;
        let substituted =
            self.substitute_user_function_argument_bindings(return_value, user_function, &[]);
        self.resolve_primitive_property_key_expression(&substituted)
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key_coercion_from_object_binding(
        &self,
        object_binding: &ObjectValueBinding,
    ) -> Option<(LocalFunctionBinding, Expression)> {
        for method_name in ["toString", "valueOf"] {
            let method_value = object_binding_lookup_value(
                object_binding,
                &Expression::String(method_name.to_string()),
            );
            match method_value {
                None | Some(Expression::Null) | Some(Expression::Undefined) => continue,
                Some(value) => {
                    let binding = self.resolve_function_binding_from_expression(value)?;
                    let key = self.resolve_property_key_from_function_binding(&binding)?;
                    return Some((binding, key));
                }
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key_expression_with_coercion(
        &self,
        expression: &Expression,
    ) -> Option<ResolvedPropertyKey> {
        if let Some(key) = self.resolve_primitive_property_key_expression(expression) {
            return Some(ResolvedPropertyKey {
                key,
                coercion: None,
            });
        }

        let object_binding = match expression {
            Expression::Object(_) => None,
            _ => self.resolve_object_binding_from_expression(expression),
        }
        .or_else(|| {
            let materialized = self.materialize_static_expression(expression);
            match materialized {
                Expression::Object(_) => None,
                _ => self.resolve_object_binding_from_expression(&materialized),
            }
        })?;
        let (coercion, key) =
            self.resolve_property_key_coercion_from_object_binding(&object_binding)?;
        Some(ResolvedPropertyKey {
            key,
            coercion: Some(coercion),
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_property_key_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        self.resolve_property_key_expression_with_coercion(expression)
            .map(|resolved| resolved.key)
    }

    pub(in crate::backend::direct_wasm) fn resolve_registered_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.module
            .registered_function_declarations
            .iter()
            .find(|function| function.name == function_name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_primitive_expression_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_primitive_expression_with_context(
                &materialized,
                current_function_name,
            );
        }

        match expression {
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined => Some(expression.clone()),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let branch = if self.resolve_static_if_condition_value(condition)? {
                    then_expression
                } else {
                    else_expression
                };
                self.resolve_static_primitive_expression_with_context(branch, current_function_name)
            }
            Expression::Identifier(name)
                if name == "undefined" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(Expression::Undefined)
            }
            Expression::Identifier(name)
                if name == "NaN" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(Expression::Number(f64::NAN))
            }
            Expression::Identifier(name)
                if name == "Infinity" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(Expression::Number(f64::INFINITY))
            }
            Expression::Member { .. } if self.resolve_static_number_value(expression).is_some() => {
                Some(Expression::Number(
                    self.resolve_static_number_value(expression)?,
                ))
            }
            Expression::Unary {
                op: UnaryOp::TypeOf,
                expression,
            } => Some(Expression::String(
                self.infer_typeof_operand_kind(expression)?
                    .as_typeof_str()?
                    .to_string(),
            )),
            Expression::Unary {
                op: UnaryOp::Negate,
                expression,
            } if self.infer_value_kind(expression) == Some(StaticValueKind::BigInt) => Some(
                Expression::BigInt((-self.resolve_static_bigint_value(expression)?).to_string()),
            ),
            Expression::Unary {
                op: UnaryOp::Plus | UnaryOp::Negate,
                ..
            }
            | Expression::Binary {
                op:
                    BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Modulo
                    | BinaryOp::Exponentiate
                    | BinaryOp::BitwiseAnd
                    | BinaryOp::BitwiseOr
                    | BinaryOp::BitwiseXor
                    | BinaryOp::LeftShift
                    | BinaryOp::RightShift
                    | BinaryOp::UnsignedRightShift,
                ..
            } => Some(Expression::Number(
                self.resolve_static_number_value(expression)?,
            )),
            Expression::Binary {
                op:
                    BinaryOp::LessThan
                    | BinaryOp::LessThanOrEqual
                    | BinaryOp::GreaterThan
                    | BinaryOp::GreaterThanOrEqual
                    | BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::LooseEqual
                    | BinaryOp::LooseNotEqual
                    | BinaryOp::In
                    | BinaryOp::InstanceOf,
                ..
            }
            | Expression::Unary {
                op: UnaryOp::Not | UnaryOp::Delete,
                ..
            } => Some(Expression::Bool(
                self.resolve_static_boolean_expression(expression)?,
            )),
            Expression::Binary {
                op: BinaryOp::Add,
                left,
                right,
            } => match self.resolve_static_addition_outcome_with_context(
                left,
                right,
                current_function_name,
            )? {
                StaticEvalOutcome::Value(value) => self
                    .resolve_static_primitive_expression_with_context(
                        &value,
                        current_function_name,
                    ),
                StaticEvalOutcome::Throw(_) => None,
            },
            Expression::Call { callee, arguments } => {
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && let Expression::String(property_name) = property.as_ref()
                    && let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_member_call_outcome_with_context(
                            object,
                            property_name,
                            current_function_name,
                        )
                {
                    return self.resolve_static_primitive_expression_with_context(
                        &value,
                        current_function_name,
                    );
                }
                let (value, callee_function_name) = self
                    .resolve_static_call_result_expression_with_context(
                        callee,
                        arguments,
                        current_function_name,
                    )?;
                self.resolve_static_primitive_expression_with_context(
                    &value,
                    callee_function_name.as_deref().or(current_function_name),
                )
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_function_outcome_from_binding_with_context(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[CallArgument],
        _current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.module.user_function_map.get(function_name)?;
        if let Some(summary) = user_function.inline_summary.as_ref()
            && summary.effects.is_empty()
            && let Some(return_value) = summary.return_value.as_ref()
        {
            return Some(StaticEvalOutcome::Value(
                self.substitute_user_function_argument_bindings(
                    return_value,
                    user_function,
                    arguments,
                ),
            ));
        }

        let function = self.resolve_registered_function_declaration(function_name)?;
        let [statement] = function.body.as_slice() else {
            return None;
        };
        match statement {
            Statement::Return(expression) => Some(StaticEvalOutcome::Value(
                self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    arguments,
                ),
            )),
            Statement::Throw(expression) => Some(StaticEvalOutcome::Throw(
                StaticThrowValue::Value(self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    arguments,
                )),
            )),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_date_timestamp(
        &self,
        expression: &Expression,
    ) -> Option<f64> {
        let resolved = self.resolve_bound_alias_expression(expression)?;
        let Expression::New { callee, arguments } = resolved else {
            return None;
        };
        let Expression::Identifier(name) = callee.as_ref() else {
            return None;
        };
        if name != "Date" {
            return None;
        }
        match arguments.first() {
            Some(CallArgument::Expression(argument)) | Some(CallArgument::Spread(argument)) => {
                self.resolve_static_number_value(argument)
            }
            None => Some(0.0),
        }
    }

    pub(in crate::backend::direct_wasm) fn synthesize_static_date_string(
        &self,
        timestamp: f64,
    ) -> String {
        if timestamp.fract() == 0.0 {
            format!("Date({})", timestamp as i64)
        } else {
            format!("Date({timestamp})")
        }
    }

    pub(in crate::backend::direct_wasm) fn synthesize_static_function_to_string(
        &self,
        function_name: &str,
    ) -> String {
        let Some(function) = self.resolve_registered_function_declaration(function_name) else {
            return format!("function {function_name}() {{}}");
        };
        let params = function
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let prefix = match function.kind {
            FunctionKind::Ordinary => "function",
            FunctionKind::Generator => "function*",
            FunctionKind::Async => "async function",
        };
        let display_name = function_display_name(function);
        match display_name {
            Some(name) if !name.is_empty() => format!("{prefix} {name}({params}) {{}}"),
            _ => format!("{prefix}({params}) {{}}"),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_symbol_to_string_value_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        if let Some(resolved) = self.resolve_bound_alias_expression(expression)
            && !static_expression_matches(&resolved, expression)
        {
            return self.resolve_static_symbol_to_string_value_with_context(
                &resolved,
                current_function_name,
            );
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_static_symbol_to_string_value_with_context(
                &materialized,
                current_function_name,
            );
        }

        if let Some(symbol_name) = self.well_known_symbol_name(expression) {
            return Some(symbol_name);
        }

        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        if !matches!(callee.as_ref(), Expression::Identifier(name) if name == "Symbol" && self.is_unshadowed_builtin_identifier(name))
        {
            return None;
        }

        let description = match arguments.first() {
            None => String::new(),
            Some(CallArgument::Expression(argument) | CallArgument::Spread(argument)) => {
                if matches!(
                    self.resolve_static_primitive_expression_with_context(
                        argument,
                        current_function_name,
                    ),
                    Some(Expression::Undefined)
                ) {
                    String::new()
                } else {
                    self.resolve_static_string_concat_value(argument, current_function_name)?
                }
            }
        };

        Some(format!("Symbol({description})"))
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_boxed_primitive_value(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let resolved = self
            .resolve_bound_alias_expression(expression)
            .unwrap_or_else(|| expression.clone());
        let (callee, arguments) = match resolved {
            Expression::New { callee, arguments } | Expression::Call { callee, arguments } => {
                (callee, arguments)
            }
            _ => return None,
        };
        let Expression::Identifier(name) = callee.as_ref() else {
            return None;
        };
        if !self.is_unshadowed_builtin_identifier(name) {
            return None;
        }
        match name.as_str() {
            "Boolean" => {
                let value = match arguments.first() {
                    Some(CallArgument::Expression(argument))
                    | Some(CallArgument::Spread(argument)) => {
                        self.resolve_static_boolean_expression(argument)?
                    }
                    None => false,
                };
                Some(Expression::Bool(value))
            }
            "Number" => {
                let value = match arguments.first() {
                    Some(CallArgument::Expression(argument))
                    | Some(CallArgument::Spread(argument)) => {
                        self.resolve_static_number_value(argument)?
                    }
                    None => 0.0,
                };
                Some(Expression::Number(value))
            }
            "String" => {
                let value = match arguments.first() {
                    Some(CallArgument::Expression(argument))
                    | Some(CallArgument::Spread(argument)) => self
                        .resolve_static_string_concat_value(
                            argument,
                            self.current_user_function_name.as_deref(),
                        )?,
                    None => String::new(),
                };
                Some(Expression::String(value))
            }
            "Object" => match arguments.first() {
                Some(CallArgument::Expression(argument)) | Some(CallArgument::Spread(argument)) => {
                    self.resolve_static_primitive_expression_with_context(
                        argument,
                        self.current_user_function_name.as_deref(),
                    )
                    .filter(|value| {
                        matches!(
                            value,
                            Expression::Number(_)
                                | Expression::BigInt(_)
                                | Expression::String(_)
                                | Expression::Bool(_)
                        )
                    })
                }
                None => None,
            },
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_symbol_to_primitive_outcome_with_context(
        &self,
        expression: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let symbol_property = symbol_to_primitive_expression();
        let default_argument = [CallArgument::Expression(Expression::String(
            "default".to_string(),
        ))];
        let call_result = if let Some(getter_binding) =
            self.resolve_member_getter_binding(expression, &symbol_property)
        {
            match self.resolve_static_function_outcome_from_binding_with_context(
                &getter_binding,
                &[],
                current_function_name,
            )? {
                StaticEvalOutcome::Throw(throw_value) => {
                    return Some(StaticEvalOutcome::Throw(throw_value));
                }
                StaticEvalOutcome::Value(method_value) => {
                    if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                        &method_value,
                        current_function_name,
                    ) {
                        return match primitive {
                            Expression::Null | Expression::Undefined => None,
                            _ => Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                                "TypeError",
                            ))),
                        };
                    }
                    let Some(binding) = self.resolve_function_binding_from_expression_with_context(
                        &method_value,
                        current_function_name,
                    ) else {
                        return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                            "TypeError",
                        )));
                    };
                    self.resolve_static_function_outcome_from_binding_with_context(
                        &binding,
                        &default_argument,
                        current_function_name,
                    )?
                }
            }
        } else if let Some(function_binding) =
            self.resolve_member_function_binding(expression, &symbol_property)
        {
            self.resolve_static_function_outcome_from_binding_with_context(
                &function_binding,
                &default_argument,
                current_function_name,
            )?
        } else {
            let object_binding = self.resolve_object_binding_from_expression(expression)?;
            let method_value = object_binding_lookup_value(&object_binding, &symbol_property)?;
            if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                method_value,
                current_function_name,
            ) {
                return match primitive {
                    Expression::Null | Expression::Undefined => None,
                    _ => Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                        "TypeError",
                    ))),
                };
            }
            let Some(binding) = self.resolve_function_binding_from_expression_with_context(
                method_value,
                current_function_name,
            ) else {
                return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                    "TypeError",
                )));
            };
            self.resolve_static_function_outcome_from_binding_with_context(
                &binding,
                &default_argument,
                current_function_name,
            )?
        };

        match call_result {
            StaticEvalOutcome::Throw(throw_value) => Some(StaticEvalOutcome::Throw(throw_value)),
            StaticEvalOutcome::Value(value) => {
                if let Some(primitive) = self
                    .resolve_static_primitive_expression_with_context(&value, current_function_name)
                {
                    return Some(StaticEvalOutcome::Value(primitive));
                }
                match self.infer_value_kind(&value) {
                    Some(StaticValueKind::Object) | Some(StaticValueKind::Function) => Some(
                        StaticEvalOutcome::Throw(StaticThrowValue::NamedError("TypeError")),
                    ),
                    _ => None,
                }
            }
        }
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

    pub(in crate::backend::direct_wasm) fn resolve_static_member_call_outcome_with_context(
        &self,
        object: &Expression,
        property_name: &str,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let property = Expression::String(property_name.to_string());
        if let Some(object_binding) = self.resolve_object_binding_from_expression(object)
            && let Some(method_value) = object_binding_lookup_value(&object_binding, &property)
        {
            let binding = self.resolve_function_binding_from_expression_with_context(
                method_value,
                current_function_name,
            )?;
            return self.resolve_static_function_outcome_from_binding_with_context(
                &binding,
                &[],
                current_function_name,
            );
        }

        if let Some(value) = self.resolve_static_boxed_primitive_value(object) {
            return match property_name {
                "valueOf" => Some(StaticEvalOutcome::Value(value)),
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    self.resolve_static_string_concat_value(&value, current_function_name)?,
                ))),
                _ => None,
            };
        }

        if let Some(symbol_text) =
            self.resolve_static_symbol_to_string_value_with_context(object, current_function_name)
        {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(symbol_text))),
                "valueOf" => Some(StaticEvalOutcome::Value(
                    self.resolve_bound_alias_expression(object)
                        .unwrap_or_else(|| self.materialize_static_expression(object)),
                )),
                _ => None,
            };
        }

        if let Some(timestamp) = self.resolve_static_date_timestamp(object) {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    self.synthesize_static_date_string(timestamp),
                ))),
                "valueOf" => Some(StaticEvalOutcome::Value(Expression::Number(timestamp))),
                _ => None,
            };
        }

        if let Some(binding) = self
            .resolve_function_binding_from_expression_with_context(object, current_function_name)
            && let LocalFunctionBinding::User(function_name) = binding
        {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    self.synthesize_static_function_to_string(&function_name),
                ))),
                _ => None,
            };
        }

        if self
            .resolve_object_binding_from_expression(object)
            .is_some()
        {
            return match property_name {
                "toString" => Some(StaticEvalOutcome::Value(Expression::String(
                    "[object Object]".to_string(),
                ))),
                _ => None,
            };
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_to_primitive_outcome_with_context(
        &self,
        expression: &Expression,
        hint: PrimitiveHint,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let resolved = self
            .resolve_bound_alias_expression(expression)
            .unwrap_or_else(|| expression.clone());
        if let Some(primitive) =
            self.resolve_static_primitive_expression_with_context(&resolved, current_function_name)
        {
            return Some(StaticEvalOutcome::Value(primitive));
        }

        if let Some(outcome) = self.resolve_static_symbol_to_primitive_outcome_with_context(
            expression,
            current_function_name,
        ) {
            return Some(outcome);
        }
        if !static_expression_matches(&resolved, expression)
            && let Some(outcome) = self.resolve_static_symbol_to_primitive_outcome_with_context(
                &resolved,
                current_function_name,
            )
        {
            return Some(outcome);
        }
        if self.symbol_to_primitive_requires_runtime_with_context(expression, current_function_name)
            || (!static_expression_matches(&resolved, expression)
                && self.symbol_to_primitive_requires_runtime_with_context(
                    &resolved,
                    current_function_name,
                ))
        {
            return None;
        }

        let coercion_target = if matches!(expression, Expression::Identifier(_)) {
            expression
        } else {
            &resolved
        };

        let prefers_string = matches!(hint, PrimitiveHint::Default)
            && self
                .resolve_static_date_timestamp(coercion_target)
                .is_some();
        let method_order = if prefers_string {
            ["toString", "valueOf"]
        } else {
            ["valueOf", "toString"]
        };

        for method_name in method_order {
            let outcome = self.resolve_static_member_call_outcome_with_context(
                coercion_target,
                method_name,
                current_function_name,
            );
            match outcome {
                Some(StaticEvalOutcome::Value(value)) => {
                    if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                        &value,
                        current_function_name,
                    ) {
                        return Some(StaticEvalOutcome::Value(primitive));
                    }
                }
                Some(StaticEvalOutcome::Throw(throw_value)) => {
                    return Some(StaticEvalOutcome::Throw(throw_value));
                }
                None => continue,
            }
        }

        if self
            .resolve_object_binding_from_expression(coercion_target)
            .is_some()
            || self
                .resolve_static_date_timestamp(coercion_target)
                .is_some()
            || self
                .resolve_function_binding_from_expression_with_context(
                    coercion_target,
                    current_function_name,
                )
                .is_some()
        {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                "TypeError",
            )));
        }

        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_addition_outcome_with_context(
        &self,
        left: &Expression,
        right: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        if self.addition_operand_requires_runtime_value(left)
            || self.addition_operand_requires_runtime_value(right)
        {
            return None;
        }
        let left_primitive = self.resolve_static_to_primitive_outcome_with_context(
            left,
            PrimitiveHint::Default,
            current_function_name,
        )?;
        let right_primitive = self.resolve_static_to_primitive_outcome_with_context(
            right,
            PrimitiveHint::Default,
            current_function_name,
        )?;
        let (left_value, right_value) = match (left_primitive, right_primitive) {
            (StaticEvalOutcome::Throw(throw_value), _)
            | (_, StaticEvalOutcome::Throw(throw_value)) => {
                return Some(StaticEvalOutcome::Throw(throw_value));
            }
            (StaticEvalOutcome::Value(left_value), StaticEvalOutcome::Value(right_value)) => {
                (left_value, right_value)
            }
        };

        if self.infer_value_kind(&left_value) == Some(StaticValueKind::String)
            || self.infer_value_kind(&right_value) == Some(StaticValueKind::String)
        {
            return Some(StaticEvalOutcome::Value(Expression::String(format!(
                "{}{}",
                self.resolve_static_string_concat_value(&left_value, current_function_name)?,
                self.resolve_static_string_concat_value(&right_value, current_function_name)?,
            ))));
        }

        let left_kind = self.infer_value_kind(&left_value);
        let right_kind = self.infer_value_kind(&right_value);
        if left_kind == Some(StaticValueKind::BigInt) && right_kind == Some(StaticValueKind::BigInt)
        {
            return Some(StaticEvalOutcome::Value(Expression::BigInt(
                (self.resolve_static_bigint_value(&left_value)?
                    + self.resolve_static_bigint_value(&right_value)?)
                .to_string(),
            )));
        }
        if left_kind == Some(StaticValueKind::BigInt) || right_kind == Some(StaticValueKind::BigInt)
        {
            return Some(StaticEvalOutcome::Throw(StaticThrowValue::NamedError(
                "TypeError",
            )));
        }

        Some(StaticEvalOutcome::Value(Expression::Number(
            self.resolve_static_number_value(&left_value)?
                + self.resolve_static_number_value(&right_value)?,
        )))
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

    pub(in crate::backend::direct_wasm) fn resolve_static_string_addition_value_with_context(
        &self,
        left: &Expression,
        right: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<String> {
        let left_primitive = match self.resolve_static_to_primitive_outcome_with_context(
            left,
            PrimitiveHint::Default,
            current_function_name,
        )? {
            StaticEvalOutcome::Value(value) => value,
            StaticEvalOutcome::Throw(_) => return None,
        };
        let right_primitive = match self.resolve_static_to_primitive_outcome_with_context(
            right,
            PrimitiveHint::Default,
            current_function_name,
        )? {
            StaticEvalOutcome::Value(value) => value,
            StaticEvalOutcome::Throw(_) => return None,
        };

        if self.infer_value_kind(&left_primitive) != Some(StaticValueKind::String)
            && self.infer_value_kind(&right_primitive) != Some(StaticValueKind::String)
        {
            return None;
        }

        Some(format!(
            "{}{}",
            self.resolve_static_string_concat_value(&left_primitive, current_function_name)?,
            self.resolve_static_string_concat_value(&right_primitive, current_function_name)?,
        ))
    }

    pub(in crate::backend::direct_wasm) fn emit_property_key_expression_effects(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<Option<Expression>> {
        let resolved = self.resolve_property_key_expression_with_coercion(expression);
        self.emit_numeric_expression(expression)?;
        self.instructions.push(0x1a);

        if let Some(binding) = resolved
            .as_ref()
            .and_then(|resolved| resolved.coercion.clone())
        {
            match binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) =
                        self.module.user_function_map.get(&function_name).cloned()
                    {
                        self.with_suspended_with_scopes(|compiler| {
                            if compiler.emit_inline_user_function_summary_with_arguments(
                                &user_function,
                                &[],
                            )? {
                                compiler.instructions.push(0x1a);
                            } else {
                                compiler.emit_user_function_call(&user_function, &[])?;
                                compiler.instructions.push(0x1a);
                            }
                            Ok(())
                        })?;
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    self.with_suspended_with_scopes(|compiler| {
                        if compiler.emit_builtin_call(&function_name, &[])? {
                            compiler.instructions.push(0x1a);
                        }
                        Ok(())
                    })?;
                }
            }
        }

        Ok(resolved.map(|resolved| resolved.key))
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_static_return_expression(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.module.user_function_map.get(function_name)?;
        let summary = user_function.inline_summary.as_ref()?;
        let return_value = summary.return_value.as_ref()?;
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        Some(self.substitute_user_function_argument_bindings(
            return_value,
            user_function,
            &call_arguments,
        ))
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_static_return_bool(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
    ) -> Option<bool> {
        self.resolve_function_binding_static_return_expression(binding, arguments)
            .and_then(|expression| self.resolve_static_boolean_expression(&expression))
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_static_return_object_binding(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
    ) -> Option<ObjectValueBinding> {
        let expression =
            self.resolve_function_binding_static_return_expression(binding, arguments)?;
        self.resolve_object_binding_from_expression(&expression)
    }

    pub(in crate::backend::direct_wasm) fn emit_function_binding_side_effects_with_arguments(
        &mut self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
    ) -> DirectResult<()> {
        self.with_suspended_with_scopes(|compiler| match binding {
            LocalFunctionBinding::User(function_name) => {
                let Some(user_function) = compiler
                    .module
                    .user_function_map
                    .get(function_name)
                    .cloned()
                else {
                    return Ok(());
                };
                if compiler
                    .emit_inline_user_function_summary_with_arguments(&user_function, arguments)?
                {
                    compiler.instructions.push(0x1a);
                } else {
                    let call_arguments = arguments
                        .iter()
                        .cloned()
                        .map(CallArgument::Expression)
                        .collect::<Vec<_>>();
                    compiler.emit_user_function_call(&user_function, &call_arguments)?;
                    compiler.instructions.push(0x1a);
                }
                Ok(())
            }
            LocalFunctionBinding::Builtin(function_name) => {
                let call_arguments = arguments
                    .iter()
                    .cloned()
                    .map(CallArgument::Expression)
                    .collect::<Vec<_>>();
                if compiler.emit_builtin_call(function_name, &call_arguments)? {
                    compiler.instructions.push(0x1a);
                }
                Ok(())
            }
        })
    }

    pub(in crate::backend::direct_wasm) fn function_binding_defaults_to_undefined(
        &self,
        binding: &LocalFunctionBinding,
    ) -> bool {
        let LocalFunctionBinding::User(function_name) = binding else {
            return false;
        };
        self.module
            .user_function_map
            .get(function_name)
            .and_then(|user_function| user_function.inline_summary.as_ref())
            .is_some_and(|summary| summary.return_value.is_none())
    }

    pub(in crate::backend::direct_wasm) fn function_binding_always_throws(
        &self,
        binding: &LocalFunctionBinding,
    ) -> bool {
        let LocalFunctionBinding::User(function_name) = binding else {
            return false;
        };
        self.resolve_registered_function_declaration(function_name)
            .is_some_and(|function| matches!(function.body.as_slice(), [Statement::Throw(_)]))
    }

    pub(in crate::backend::direct_wasm) fn resolve_terminal_function_outcome_from_binding(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
    ) -> Option<StaticEvalOutcome> {
        if let Some(outcome) = self.resolve_static_function_outcome_from_binding_with_context(
            binding,
            &arguments
                .iter()
                .cloned()
                .map(CallArgument::Expression)
                .collect::<Vec<_>>(),
            self.current_user_function_name.as_deref(),
        ) {
            return Some(outcome);
        }
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.module.user_function_map.get(function_name)?;
        let function = self.resolve_registered_function_declaration(function_name)?;
        let terminal_statement = function.body.last()?;
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        match terminal_statement {
            Statement::Return(expression) => Some(StaticEvalOutcome::Value(
                self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    &call_arguments,
                ),
            )),
            Statement::Throw(expression) => Some(StaticEvalOutcome::Throw(
                StaticThrowValue::Value(self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    &call_arguments,
                )),
            )),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_terminal_call_expression_outcome(
        &self,
        expression: &Expression,
    ) -> Option<StaticEvalOutcome> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let binding = self.resolve_function_binding_from_expression(callee)?;
        let argument_expressions = arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) => Some(expression.clone()),
                CallArgument::Spread(_) => None,
            })
            .collect::<Option<Vec<_>>>()?;
        self.resolve_terminal_function_outcome_from_binding(&binding, &argument_expressions)
    }

    pub(in crate::backend::direct_wasm) fn resolve_effectful_returned_object_binding(
        &self,
        expression: &Expression,
    ) -> Option<ObjectValueBinding> {
        let (callee, arguments) = match expression {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };
        let binding = self.resolve_function_binding_from_expression(callee)?;
        let argument_expressions = arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) => Some(expression.clone()),
                CallArgument::Spread(_) => None,
            })
            .collect::<Option<Vec<_>>>()?;
        self.resolve_function_binding_static_return_object_binding(&binding, &argument_expressions)
    }

    pub(in crate::backend::direct_wasm) fn resolve_ordinary_to_primitive_plan(
        &self,
        expression: &Expression,
    ) -> Option<OrdinaryToPrimitivePlan> {
        let object_binding = self
            .resolve_object_binding_from_expression(expression)
            .or_else(|| self.resolve_effectful_returned_object_binding(expression))?;
        let mut steps = Vec::new();
        for method_name in ["valueOf", "toString"] {
            let property = Expression::String(method_name.to_string());
            let Some(method_value) = object_binding_lookup_value(&object_binding, &property) else {
                continue;
            };
            let binding = self.resolve_function_binding_from_expression(method_value)?;
            let outcome = self.resolve_terminal_function_outcome_from_binding(&binding, &[])?;
            steps.push(OrdinaryToPrimitiveStep { binding, outcome });
        }
        (!steps.is_empty()).then_some(OrdinaryToPrimitivePlan { steps })
    }

    pub(in crate::backend::direct_wasm) fn static_expression_is_non_object_primitive(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        match self.infer_value_kind(expression)? {
            StaticValueKind::Number
            | StaticValueKind::BigInt
            | StaticValueKind::String
            | StaticValueKind::Bool
            | StaticValueKind::Null
            | StaticValueKind::Undefined
            | StaticValueKind::Symbol => Some(true),
            StaticValueKind::Object | StaticValueKind::Function => Some(false),
            StaticValueKind::Unknown => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn analyze_ordinary_to_primitive_plan(
        &self,
        plan: &OrdinaryToPrimitivePlan,
    ) -> OrdinaryToPrimitiveAnalysis {
        for step in &plan.steps {
            match &step.outcome {
                StaticEvalOutcome::Throw(_) => return OrdinaryToPrimitiveAnalysis::Throw,
                StaticEvalOutcome::Value(value) => {
                    match self.static_expression_is_non_object_primitive(value) {
                        Some(true) => {
                            if let Some(kind) = self.infer_value_kind(value) {
                                return OrdinaryToPrimitiveAnalysis::Primitive(kind);
                            }
                            return OrdinaryToPrimitiveAnalysis::Unknown;
                        }
                        Some(false) => continue,
                        None => return OrdinaryToPrimitiveAnalysis::Unknown,
                    }
                }
            }
        }
        OrdinaryToPrimitiveAnalysis::TypeError
    }

    pub(in crate::backend::direct_wasm) fn emit_ordinary_to_primitive_from_plan(
        &mut self,
        expression: &Expression,
        plan: &OrdinaryToPrimitivePlan,
        result_local: u32,
    ) -> DirectResult<SymbolToPrimitiveHandling> {
        for step in &plan.steps {
            if !self.emit_binding_call_result_to_local_with_explicit_this(
                &step.binding,
                &[],
                expression,
                JS_TYPEOF_OBJECT_TAG,
                result_local,
            )? {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            match &step.outcome {
                StaticEvalOutcome::Throw(_) => return Ok(SymbolToPrimitiveHandling::AlwaysThrows),
                StaticEvalOutcome::Value(value) => {
                    match self.static_expression_is_non_object_primitive(value) {
                        Some(true) => return Ok(SymbolToPrimitiveHandling::Handled),
                        Some(false) => continue,
                        None => return Ok(SymbolToPrimitiveHandling::NotHandled),
                    }
                }
            }
        }
        self.emit_named_error_throw("TypeError")?;
        Ok(SymbolToPrimitiveHandling::AlwaysThrows)
    }

    pub(in crate::backend::direct_wasm) fn emit_effectful_ordinary_to_primitive_addition(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<bool> {
        let left_plan = self.resolve_ordinary_to_primitive_plan(left);
        let right_plan = self.resolve_ordinary_to_primitive_plan(right);
        let left_eval_throw = matches!(
            self.resolve_terminal_call_expression_outcome(left),
            Some(StaticEvalOutcome::Throw(_))
        );
        let right_eval_throw = matches!(
            self.resolve_terminal_call_expression_outcome(right),
            Some(StaticEvalOutcome::Throw(_))
        );
        let left_analysis = left_plan
            .as_ref()
            .map(|plan| self.analyze_ordinary_to_primitive_plan(plan))
            .unwrap_or(OrdinaryToPrimitiveAnalysis::Unknown);
        let right_analysis = right_plan
            .as_ref()
            .map(|plan| self.analyze_ordinary_to_primitive_plan(plan))
            .unwrap_or(OrdinaryToPrimitiveAnalysis::Unknown);

        let final_type_error = matches!(
            (left_analysis, right_analysis),
            (
                OrdinaryToPrimitiveAnalysis::Primitive(StaticValueKind::Symbol),
                _
            ) | (
                _,
                OrdinaryToPrimitiveAnalysis::Primitive(StaticValueKind::Symbol)
            ) | (OrdinaryToPrimitiveAnalysis::TypeError, _)
                | (_, OrdinaryToPrimitiveAnalysis::TypeError)
        );

        if !(left_eval_throw
            || right_eval_throw
            || matches!(left_analysis, OrdinaryToPrimitiveAnalysis::Throw)
            || matches!(right_analysis, OrdinaryToPrimitiveAnalysis::Throw)
            || final_type_error)
        {
            return Ok(false);
        }

        let left_local = self.allocate_temp_local();
        self.emit_numeric_expression(left)?;
        self.push_local_set(left_local);
        if left_eval_throw {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        let right_local = self.allocate_temp_local();
        self.emit_numeric_expression(right)?;
        self.push_local_set(right_local);
        if right_eval_throw {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        if let Some(plan) = left_plan.as_ref() {
            match self.emit_ordinary_to_primitive_from_plan(left, plan, left_local)? {
                SymbolToPrimitiveHandling::AlwaysThrows => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                }
                SymbolToPrimitiveHandling::Handled => {}
                SymbolToPrimitiveHandling::NotHandled => return Ok(false),
            }
        }

        if let Some(plan) = right_plan.as_ref() {
            match self.emit_ordinary_to_primitive_from_plan(right, plan, right_local)? {
                SymbolToPrimitiveHandling::AlwaysThrows => {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                }
                SymbolToPrimitiveHandling::Handled => {}
                SymbolToPrimitiveHandling::NotHandled => return Ok(false),
            }
        }

        if final_type_error {
            self.emit_named_error_throw("TypeError")?;
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_binding_call_result_to_local_with_explicit_this(
        &mut self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
        this_expression: &Expression,
        this_value: i32,
        result_local: u32,
    ) -> DirectResult<bool> {
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        match binding {
            LocalFunctionBinding::User(function_name) => {
                let Some(user_function) = self.module.user_function_map.get(function_name).cloned()
                else {
                    return Ok(false);
                };
                if self.emit_inline_user_function_summary_with_explicit_call_frame(
                    &user_function,
                    arguments,
                    this_expression,
                    result_local,
                )? {
                    return Ok(true);
                }
                self.emit_user_function_call_with_new_target_and_this(
                    &user_function,
                    &call_arguments,
                    JS_UNDEFINED_TAG,
                    this_value,
                )?;
                self.push_local_set(result_local);
                Ok(true)
            }
            LocalFunctionBinding::Builtin(function_name) => {
                if !self.emit_builtin_call(function_name, &call_arguments)? {
                    return Ok(false);
                }
                self.push_local_set(result_local);
                Ok(true)
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_effectful_symbol_to_primitive_for_operand(
        &mut self,
        expression: &Expression,
        default_argument: &Expression,
    ) -> DirectResult<SymbolToPrimitiveHandling> {
        let symbol_property = symbol_to_primitive_expression();
        if let Some(getter_binding) =
            self.resolve_member_getter_binding(expression, &symbol_property)
        {
            let getter_result_local = self.allocate_temp_local();
            if !self.emit_binding_call_result_to_local_with_explicit_this(
                &getter_binding,
                &[],
                expression,
                JS_TYPEOF_OBJECT_TAG,
                getter_result_local,
            )? {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            if self.function_binding_always_throws(&getter_binding) {
                return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
            }
            if let Some(return_expression) = self
                .resolve_function_binding_static_return_expression_with_call_frame(
                    &getter_binding,
                    &[],
                    expression,
                )
            {
                if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                    &return_expression,
                    self.current_user_function_name.as_deref(),
                ) {
                    if matches!(primitive, Expression::Null | Expression::Undefined) {
                        return Ok(SymbolToPrimitiveHandling::Handled);
                    }
                    self.emit_named_error_throw("TypeError")?;
                    return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
                }
                if let Some(return_binding) =
                    self.resolve_function_binding_from_expression(&return_expression)
                {
                    let return_result_local = self.allocate_temp_local();
                    if !self.emit_binding_call_result_to_local_with_explicit_this(
                        &return_binding,
                        std::slice::from_ref(default_argument),
                        expression,
                        JS_TYPEOF_OBJECT_TAG,
                        return_result_local,
                    )? {
                        return Ok(SymbolToPrimitiveHandling::NotHandled);
                    }
                    if self.function_binding_always_throws(&return_binding) {
                        return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
                    }
                    return Ok(SymbolToPrimitiveHandling::Handled);
                }
                self.emit_named_error_throw("TypeError")?;
                return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
            }
            if self.function_binding_defaults_to_undefined(&getter_binding) {
                return Ok(SymbolToPrimitiveHandling::Handled);
            }
        }

        if let Some(function_binding) = self
            .resolve_member_function_binding(expression, &symbol_property)
            .or_else(|| {
                self.resolve_object_binding_from_expression(expression)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(&object_binding, &symbol_property)
                            .and_then(|value| self.resolve_function_binding_from_expression(value))
                    })
            })
        {
            let result_local = self.allocate_temp_local();
            if !self.emit_binding_call_result_to_local_with_explicit_this(
                &function_binding,
                std::slice::from_ref(default_argument),
                expression,
                JS_TYPEOF_OBJECT_TAG,
                result_local,
            )? {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            if self.function_binding_always_throws(&function_binding) {
                return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
            }
            return Ok(SymbolToPrimitiveHandling::Handled);
        }

        if let Some(object_binding) = self.resolve_object_binding_from_expression(expression)
            && let Some(method_value) =
                object_binding_lookup_value(&object_binding, &symbol_property)
            && let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                method_value,
                self.current_user_function_name.as_deref(),
            )
        {
            if matches!(primitive, Expression::Null | Expression::Undefined) {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            self.emit_named_error_throw("TypeError")?;
            return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
        }

        Ok(SymbolToPrimitiveHandling::NotHandled)
    }

    pub(in crate::backend::direct_wasm) fn emit_effectful_symbol_to_primitive_addition(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<bool> {
        let default_argument = Expression::String("default".to_string());
        let left_handling =
            self.emit_effectful_symbol_to_primitive_for_operand(left, &default_argument)?;
        if left_handling == SymbolToPrimitiveHandling::AlwaysThrows {
            return Ok(true);
        }
        let right_handling =
            self.emit_effectful_symbol_to_primitive_for_operand(right, &default_argument)?;

        if left_handling == SymbolToPrimitiveHandling::NotHandled
            && right_handling == SymbolToPrimitiveHandling::NotHandled
        {
            return Ok(false);
        }

        if left_handling == SymbolToPrimitiveHandling::NotHandled {
            self.emit_numeric_expression(left)?;
            self.instructions.push(0x1a);
        }
        if right_handling == SymbolToPrimitiveHandling::NotHandled {
            self.emit_numeric_expression(right)?;
            self.instructions.push(0x1a);
        }

        self.push_i32_const(JS_NAN_TAG);
        Ok(true)
    }
}
