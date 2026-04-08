use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_fallback_member_call_shortcuts(
        &mut self,
        _source_expression: &Expression,
        callee: &Expression,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if matches!(property, Expression::String(property_name) if property_name == "next")
            && matches!(object, Expression::Identifier(name) if self.state.speculation.static_semantics.has_local_array_iterator_binding(name))
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }
        if matches!(property, Expression::String(property_name) if property_name == "slice")
            && self
                .resolve_array_slice_binding(object, arguments)
                .is_some()
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_ignored_call_arguments(arguments)?;
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }

        self.emit_dynamic_user_function_call(callee, arguments)
    }
}
