use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_specialized_callee_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if let Some(specialized) = self.resolve_specialized_function_value_from_expression(callee) {
            return self.emit_specialized_function_value_call(&specialized, arguments);
        }

        let Expression::Member { object, property } = callee else {
            return Ok(false);
        };
        let Some(specialized) =
            self.resolve_tracked_array_specialized_function_value(object, property)
        else {
            return Ok(false);
        };
        let Expression::Identifier(name) = object.as_ref() else {
            return Ok(false);
        };
        let Some(index) = argument_index_from_expression(property) else {
            return Ok(false);
        };
        if let Some(slot) = self.runtime_array_slot(name, index) {
            self.push_local_get(slot.present_local);
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.emit_specialized_function_value_call(&specialized, arguments)?;
            self.state.emission.output.instructions.push(0x05);
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(true);
        }
        self.emit_specialized_function_value_call(&specialized, arguments)
    }
}
