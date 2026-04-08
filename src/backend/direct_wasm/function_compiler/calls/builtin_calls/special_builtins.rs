use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_function_constructor_builtin_call(
        &mut self,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !is_function_constructor_builtin(name) {
            return Ok(false);
        }

        if let Some((parameter_source, body_source)) =
            function_constructor_literal_source_parts(arguments)
        {
            let wrappers =
                function_constructor_wrapper_sources(name, &parameter_source, &body_source)
                    .expect("checked builtin names should produce wrapper sources");
            let parses = wrappers
                .iter()
                .any(|wrapper| frontend::parse(wrapper).is_ok());
            if !parses {
                self.emit_named_error_throw("SyntaxError")?;
                return Ok(true);
            }
        }

        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_test262_realm_eval_call(
        &mut self,
        builtin_name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(realm_id) = parse_test262_realm_eval_builtin(builtin_name) else {
            return Ok(false);
        };
        let Some(argument) = arguments.first() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };

        let CallArgument::Expression(Expression::String(argument_source)) = argument else {
            return Ok(false);
        };
        let Ok(program) = frontend::parse_script_goal(argument_source) else {
            self.emit_named_error_throw("SyntaxError")?;
            return Ok(true);
        };

        let [Statement::Var { name, value }] = program.statements.as_slice() else {
            return Ok(false);
        };
        let materialized_value = self.materialize_static_expression(value);
        let Some(realm) = self.test262_realm_mut(realm_id) else {
            return Ok(false);
        };
        object_binding_set_property(
            &mut realm.global_object_binding,
            Expression::String(name.clone()),
            materialized_value,
        );

        for argument in arguments.iter().skip(1) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_is_nan_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let first_argument = arguments.first();

        if let Some(CallArgument::Expression(Expression::String(text))) = first_argument {
            for argument in arguments.iter().skip(1) {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(if parse_string_to_i32(text).is_ok() {
                0
            } else {
                1
            });
            return Ok(true);
        }

        if matches!(
            first_argument,
            Some(CallArgument::Expression(
                Expression::Object(_) | Expression::Array(_) | Expression::This
            ))
        ) {
            for argument in arguments.iter() {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(1);
            return Ok(true);
        }

        let value_local = self.allocate_temp_local();
        match first_argument {
            Some(CallArgument::Expression(expression) | CallArgument::Spread(expression)) => {
                self.emit_numeric_expression(expression)?;
            }
            None => self.push_i32_const(JS_UNDEFINED_TAG),
        }
        self.push_local_set(value_local);

        for argument in arguments.iter().skip(1) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        self.push_local_get(value_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.push_local_get(value_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::Equal)?;
        self.push_binary_op(BinaryOp::BitwiseOr)?;
        Ok(true)
    }
}
