use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_object_is_call(
        &mut self,
        callee_object: &Expression,
        callee_property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !matches!(callee_object, Expression::Identifier(name) if name == "Object" && self.is_unshadowed_builtin_identifier(name))
        {
            return Ok(false);
        }
        if !matches!(callee_property, Expression::String(name) if name == "is") {
            return Ok(false);
        }

        let [
            CallArgument::Expression(actual),
            CallArgument::Expression(expected),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };

        if let Some(result) = self.resolve_static_same_value_result_with_context(
            actual,
            expected,
            self.current_function_name(),
        ) {
            self.emit_numeric_expression(actual)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(expected)?;
            self.state.emission.output.instructions.push(0x1a);
            for argument in rest {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(result as i32);
            return Ok(true);
        }

        let actual_local = self.allocate_temp_local();
        let expected_local = self.allocate_temp_local();

        self.emit_numeric_expression(actual)?;
        self.push_local_set(actual_local);
        self.emit_numeric_expression(expected)?;
        self.push_local_set(expected_local);

        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        self.emit_same_value_result_from_locals(actual_local, expected_local, actual_local)?;
        self.push_local_get(actual_local);
        Ok(true)
    }
}
