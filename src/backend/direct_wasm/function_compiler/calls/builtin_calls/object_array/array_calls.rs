use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_array_is_array_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Array" && self.is_unshadowed_builtin_identifier(name))
        {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "isArray") {
            return Ok(false);
        }

        let Some(first_argument) = arguments.first() else {
            self.push_i32_const(0);
            return Ok(true);
        };

        let array_like = match first_argument {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                !matches!(expression, Expression::Identifier(name) if self.state.speculation.static_semantics.has_local_typed_array_view_binding(name))
                    && self
                        .resolve_array_binding_from_expression(expression)
                        .is_some()
            }
        };

        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        self.push_i32_const(array_like as i32);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_array_for_each_call(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(property, Expression::String(name) if name == "forEach") {
            return Ok(false);
        }
        let Some(array_binding) = self.resolve_array_binding_from_expression(object) else {
            return Ok(false);
        };
        let Some(CallArgument::Expression(callback)) = arguments.first() else {
            return Ok(false);
        };
        let Some(LocalFunctionBinding::User(function_name)) =
            self.resolve_function_binding_from_expression(callback)
        else {
            return Ok(false);
        };
        let Some(user_function) = self.user_function(&function_name).cloned() else {
            return Ok(false);
        };

        let this_expression = match arguments.get(1) {
            Some(CallArgument::Expression(expression)) | Some(CallArgument::Spread(expression)) => {
                expression.clone()
            }
            None => Expression::Undefined,
        };
        let this_hidden_name =
            self.allocate_named_hidden_local("array_foreach_this", StaticValueKind::Unknown);
        let this_hidden_local = self
            .state
            .runtime
            .locals
            .get(&this_hidden_name)
            .copied()
            .expect("forEach this hidden local should exist");
        let array_hidden_name =
            self.allocate_named_hidden_local("array_foreach_array", StaticValueKind::Object);
        let array_hidden_local = self
            .state
            .runtime
            .locals
            .get(&array_hidden_name)
            .copied()
            .expect("forEach array hidden local should exist");

        self.emit_numeric_expression(object)?;
        self.push_local_set(array_hidden_local);
        self.emit_numeric_expression(callback)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(&this_expression)?;
        self.push_local_set(this_hidden_local);
        for argument in arguments.iter().skip(2) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        for (index, value) in array_binding.values.iter().enumerate() {
            let Some(value) = value.clone() else {
                continue;
            };
            let callback_arguments = [
                CallArgument::Expression(value),
                CallArgument::Expression(Expression::Number(index as f64)),
                CallArgument::Expression(Expression::Identifier(array_hidden_name.clone())),
            ];
            self.emit_user_function_call_with_new_target_and_this_expression(
                &user_function,
                &callback_arguments,
                JS_UNDEFINED_TAG,
                &Expression::Identifier(this_hidden_name.clone()),
            )?;
            self.state.emission.output.instructions.push(0x1a);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }
}
