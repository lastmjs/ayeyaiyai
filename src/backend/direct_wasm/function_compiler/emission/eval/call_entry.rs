use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_eval_statement_completion_value(
        &mut self,
        statement: &Statement,
        completion_local: u32,
    ) -> DirectResult<()> {
        match statement {
            Statement::Expression(expression) => {
                self.emit_numeric_expression(expression)?;
                self.push_local_tee(completion_local);
                self.state.emission.output.instructions.push(0x1a);
            }
            Statement::Assign { name, value } => {
                self.emit_numeric_expression(&Expression::Assign {
                    name: name.clone(),
                    value: Box::new(value.clone()),
                })?;
                self.push_local_tee(completion_local);
                self.state.emission.output.instructions.push(0x1a);
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.emit_numeric_expression(&Expression::AssignMember {
                    object: Box::new(object.clone()),
                    property: Box::new(property.clone()),
                    value: Box::new(value.clone()),
                })?;
                self.push_local_tee(completion_local);
                self.state.emission.output.instructions.push(0x1a);
            }
            _ => self.emit_statement(statement)?,
        }

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_builtin_call_for_callee(
        &mut self,
        callee: &Expression,
        name: &str,
        arguments: &[CallArgument],
        construct: bool,
    ) -> DirectResult<bool> {
        if !construct
            && let Some(value) = self.resolve_static_builtin_primitive_call_value(
                name,
                arguments,
                self.current_function_name(),
            )
        {
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.emit_numeric_expression(&value)?;
            return Ok(true);
        }

        if name == "eval" {
            if matches!(callee, Expression::Identifier(identifier) if identifier == "eval") {
                return self.emit_eval_call(arguments);
            }
            return self.emit_indirect_eval_call(arguments);
        }

        self.emit_builtin_call(name, arguments)
    }
}
