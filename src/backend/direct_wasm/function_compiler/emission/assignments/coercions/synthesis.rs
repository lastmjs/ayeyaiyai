use super::*;

impl<'a> FunctionCompiler<'a> {
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
            FunctionKind::AsyncGenerator => "async function*",
        };
        let display_name = self.resolve_user_function_display_name(function_name);
        match display_name {
            Some(name) if !name.is_empty() => format!("{prefix} {name}({params}) {{}}"),
            _ => format!("{prefix}({params}) {{}}"),
        }
    }

    pub(in crate::backend::direct_wasm) fn synthesize_static_function_binding_to_string(
        &self,
        binding: &LocalFunctionBinding,
    ) -> String {
        match binding {
            LocalFunctionBinding::User(function_name) => {
                self.synthesize_static_function_to_string(function_name)
            }
            LocalFunctionBinding::Builtin(function_name) => format!(
                "function {}() {{}}",
                builtin_function_display_name(function_name)
            ),
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
                            self.current_function_name(),
                        )?,
                    None => String::new(),
                };
                Some(Expression::String(value))
            }
            "Object" => match arguments.first() {
                Some(CallArgument::Expression(argument)) | Some(CallArgument::Spread(argument)) => {
                    self.resolve_static_primitive_expression_with_context(
                        argument,
                        self.current_function_name(),
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
}
