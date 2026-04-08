use super::*;

#[path = "late_shortcuts/array_shortcuts.rs"]
mod array_shortcuts;
#[path = "late_shortcuts/fallback_shortcuts.rs"]
mod fallback_shortcuts;
#[path = "late_shortcuts/property_shortcuts.rs"]
mod property_shortcuts;
#[path = "late_shortcuts/returned_calls.rs"]
mod returned_calls;
#[path = "late_shortcuts/string_shortcuts.rs"]
mod string_shortcuts;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_late_member_call_shortcuts(
        &mut self,
        source_expression: &Expression,
        callee: &Expression,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if self.emit_builtin_member_call_shortcuts(callee, object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_array_member_call_shortcuts(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_property_member_call_shortcuts(
            source_expression,
            object,
            property,
            arguments,
        )? {
            return Ok(true);
        }
        if self.emit_string_member_call_shortcuts(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_returned_member_call_shortcuts(callee, object, property, arguments)? {
            return Ok(true);
        }
        self.emit_fallback_member_call_shortcuts(
            source_expression,
            callee,
            object,
            property,
            arguments,
        )
    }

    fn emit_builtin_member_call_shortcuts(
        &mut self,
        callee: &Expression,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if matches!(object, Expression::Identifier(name) if name == "assert")
            && matches!(property, Expression::String(name) if name == "compareArray")
            && self.emit_assert_compare_array_call(arguments)?
        {
            return Ok(true);
        }
        if self.emit_object_array_builtin_call(object, property, arguments)? {
            return Ok(true);
        }
        if self.emit_array_for_each_call(object, property, arguments)? {
            return Ok(true);
        }
        if matches!(property, Expression::String(property_name) if property_name == "push")
            && self.emit_tracked_array_push_call(object, arguments)?
        {
            return Ok(true);
        }
        self.emit_member_function_binding_call_expression(callee, object, property, arguments)
    }
}
