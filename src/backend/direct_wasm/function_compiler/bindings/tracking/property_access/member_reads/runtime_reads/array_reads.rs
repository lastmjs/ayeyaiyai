use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_runtime_array_member_read(
        &mut self,
        object: &Expression,
        static_array_property: &Expression,
    ) -> DirectResult<bool> {
        if let Expression::Identifier(name) = object
            && let Some(index) = argument_index_from_expression(static_array_property)
        {
            if let Some(array_binding) = self.resolve_array_binding_from_expression(object)
                && let Some(Some(value)) = array_binding.values.get(index as usize)
            {
                self.emit_numeric_expression(value)?;
                return Ok(true);
            }
            if self.emit_global_runtime_array_slot_read(name, index)? {
                return Ok(true);
            }
            if self.emit_runtime_array_slot_read(name, index)? {
                return Ok(true);
            }
        }

        let Some(array_binding) = self.resolve_array_binding_from_expression(object) else {
            return Ok(false);
        };
        if matches!(static_array_property, Expression::String(text) if text == "length") {
            if let Expression::Identifier(name) = object
                && self.emit_global_runtime_array_length_read(name)
            {
                return Ok(true);
            }
            if let Some(length_local) = self.runtime_array_length_local_for_expression(object) {
                self.push_local_get(length_local);
            } else {
                self.push_i32_const(array_binding.values.len() as i32);
            }
            return Ok(true);
        }
        if let Some(index) = argument_index_from_expression(static_array_property) {
            if let Expression::Identifier(name) = object
                && self.emit_global_runtime_array_slot_read(name, index)?
            {
                return Ok(true);
            }
            if let Some(Some(value)) = array_binding.values.get(index as usize) {
                self.emit_numeric_expression(value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(true);
        }

        Ok(false)
    }
}
