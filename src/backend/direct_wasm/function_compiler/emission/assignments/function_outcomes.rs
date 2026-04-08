use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_function_binding_static_return_expression(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[Expression],
    ) -> Option<Expression> {
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.user_function(function_name)?;
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
                let Some(user_function) = compiler.user_function(function_name).cloned() else {
                    return Ok(());
                };
                if compiler
                    .emit_inline_user_function_summary_with_arguments(&user_function, arguments)?
                {
                    compiler.state.emission.output.instructions.push(0x1a);
                } else {
                    let call_arguments = arguments
                        .iter()
                        .cloned()
                        .map(CallArgument::Expression)
                        .collect::<Vec<_>>();
                    compiler.emit_user_function_call(&user_function, &call_arguments)?;
                    compiler.state.emission.output.instructions.push(0x1a);
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
                    compiler.state.emission.output.instructions.push(0x1a);
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
        self.user_function(function_name)
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
            self.current_function_name(),
        ) {
            return Some(outcome);
        }
        let LocalFunctionBinding::User(function_name) = binding else {
            return None;
        };
        let user_function = self.user_function(function_name)?;
        let function = self.resolve_registered_function_declaration(function_name)?;
        if function.body.is_empty() {
            return Some(StaticEvalOutcome::Value(Expression::Undefined));
        }
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
            Statement::Expression(expression) => self
                .resolve_terminal_expression_throw_value(
                    &self.substitute_user_function_argument_bindings(
                        expression,
                        user_function,
                        &call_arguments,
                    ),
                )
                .map(|throw_value| StaticEvalOutcome::Throw(StaticThrowValue::Value(throw_value))),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_terminal_expression_throw_value(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        match expression {
            Expression::Call { .. } => {
                match self.resolve_terminal_call_expression_outcome(expression)? {
                    StaticEvalOutcome::Throw(throw_value) => {
                        self.resolve_static_throw_value_expression(&throw_value)
                    }
                    _ => None,
                }
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => self
                .resolve_terminal_expression_throw_value(object)
                .or_else(|| self.resolve_terminal_expression_throw_value(property))
                .or_else(|| self.resolve_terminal_expression_throw_value(value)),
            Expression::AssignSuperMember { property, value } => self
                .resolve_terminal_expression_throw_value(property)
                .or_else(|| self.resolve_terminal_expression_throw_value(value)),
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    if let Some(throw_value) =
                        self.resolve_terminal_expression_throw_value(expression)
                    {
                        return Some(throw_value);
                    }
                }
                None
            }
            Expression::Await(expression)
            | Expression::EnumerateKeys(expression)
            | Expression::GetIterator(expression)
            | Expression::IteratorClose(expression)
            | Expression::Unary { expression, .. } => {
                self.resolve_terminal_expression_throw_value(expression)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_static_throw_value_expression(
        &self,
        throw_value: &StaticThrowValue,
    ) -> Option<Expression> {
        match throw_value {
            StaticThrowValue::Value(throw_value) => Some(throw_value.clone()),
            StaticThrowValue::NamedError(name) => Some(Expression::Call {
                callee: Box::new(Expression::Identifier((*name).to_string())),
                arguments: vec![],
            }),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_terminal_call_expression_outcome(
        &self,
        expression: &Expression,
    ) -> Option<StaticEvalOutcome> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        if matches!(
            callee.as_ref(),
            Expression::Identifier(name) if name == "eval"
        ) && matches!(
            self.resolve_function_binding_from_expression(callee),
            Some(LocalFunctionBinding::Builtin(function_name)) if function_name == "eval"
        ) && let Some(outcome) = self.resolve_static_direct_eval_outcome(arguments)
        {
            return Some(outcome);
        }
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
}
