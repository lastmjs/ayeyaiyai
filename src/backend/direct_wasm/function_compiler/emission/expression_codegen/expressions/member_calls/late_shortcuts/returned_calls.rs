use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_returned_member_call_shortcuts(
        &mut self,
        callee: &Expression,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if let Some(inlined_call) =
            self.resolve_inline_call_from_returned_member(object, property, arguments)
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(&inlined_call)?;
            return Ok(true);
        }
        if let Some(returned_value) =
            self.resolve_returned_member_value_from_expression(object, property)
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            if let Some(function_binding) =
                self.resolve_function_binding_from_expression(&returned_value)
            {
                match function_binding {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) = self.user_function(&function_name).cloned() {
                            self.emit_user_function_call_with_new_target_and_this(
                                &user_function,
                                arguments,
                                JS_UNDEFINED_TAG,
                                JS_TYPEOF_OBJECT_TAG,
                            )?;
                            return Ok(true);
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        if self.emit_builtin_call_for_callee(
                            callee,
                            &function_name,
                            arguments,
                            false,
                        )? {
                            return Ok(true);
                        }
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}
