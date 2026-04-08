use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_static_weakref_deref_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let target = match callee {
            Expression::Member { object, property }
                if matches!(property.as_ref(), Expression::String(name) if name == "deref")
                    && arguments.is_empty() =>
            {
                self.emit_numeric_expression(object)?;
                self.state.emission.output.instructions.push(0x1a);
                self.resolve_static_weakref_target_expression(object)
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Expression::Member {
                    object: deref_target,
                    property: deref_property,
                } = object.as_ref()
                else {
                    return Ok(false);
                };
                if !matches!(deref_property.as_ref(), Expression::String(name) if name == "deref") {
                    return Ok(false);
                }
                self.emit_numeric_expression(deref_target)?;
                self.state.emission.output.instructions.push(0x1a);
                let target = match arguments.first() {
                    Some(CallArgument::Expression(this_expression))
                    | Some(CallArgument::Spread(this_expression)) => {
                        self.resolve_static_weakref_target_expression(this_expression)
                    }
                    None => None,
                };
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                target
            }
            _ => return Ok(false),
        };
        let Some(target) = target else {
            return Ok(false);
        };
        self.emit_numeric_expression(&target)?;
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_instanceof_truthy_from_local(
        &mut self,
        value_local: u32,
    ) -> DirectResult<()> {
        self.push_local_get(value_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::NotEqual)?;

        self.push_local_get(value_local);
        self.push_i32_const(JS_NULL_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x71);

        self.push_local_get(value_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x71);

        self.push_local_get(value_local);
        self.push_i32_const(JS_NAN_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x71);
        Ok(())
    }
}
