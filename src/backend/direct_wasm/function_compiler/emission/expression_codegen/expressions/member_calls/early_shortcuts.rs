use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_early_member_call_shortcuts(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if self.emit_immediate_promise_member_call(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_function_prototype_call_or_apply(object, property, arguments)? {
            return Ok(true);
        }
        if matches!(object, Expression::Identifier(name) if name == "assert")
            && matches!(property, Expression::String(name) if name == "sameValue")
            && self.emit_assertion_builtin_call("__assertSameValue", arguments)?
        {
            return Ok(true);
        }
        if matches!(object, Expression::Identifier(name) if name == "assert")
            && matches!(property, Expression::String(name) if name == "notSameValue")
            && self.emit_assertion_builtin_call("__assertNotSameValue", arguments)?
        {
            return Ok(true);
        }
        if self.emit_array_is_array_call(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_object_is_call(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_object_create_call(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_object_get_prototype_of_call(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_object_is_extensible_call(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_object_set_prototype_of_call(object, property, arguments)? {
            return Ok(true);
        }
        if matches!(property, Expression::String(property_name) if property_name == "resize")
            && let (
                Expression::Identifier(buffer_name),
                Some(
                    CallArgument::Expression(length_expression)
                    | CallArgument::Spread(length_expression),
                ),
            ) = (object, arguments.first())
            && let Some(new_length) = extract_typed_array_element_count(length_expression)
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(length_expression)?;
            self.state.emission.output.instructions.push(0x1a);
            for argument in arguments.iter().skip(1) {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            if self.apply_resizable_array_buffer_resize(buffer_name, new_length)? {
                self.push_i32_const(JS_UNDEFINED_TAG);
                return Ok(true);
            }
        }
        Ok(false)
    }
}
