use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_arguments_or_restricted_member_assignment(
        &mut self,
        object: &Expression,
        property: &Expression,
        value: &Expression,
    ) -> DirectResult<bool> {
        if matches!(property, Expression::String(property_name) if property_name == "callee" || property_name == "length")
        {
            let Expression::String(property_name) = property else {
                unreachable!("filtered above");
            };
            if self.is_direct_arguments_object(object) {
                let temp_local = self.allocate_temp_local();
                self.emit_numeric_expression(value)?;
                self.push_local_set(temp_local);
                if property_name == "callee" && self.state.speculation.execution_context.strict_mode
                {
                    self.push_local_get(temp_local);
                    self.state.emission.output.instructions.push(0x1a);
                    return self.emit_error_throw().map(|_| true);
                }
                self.apply_current_arguments_effect(
                    property_name,
                    ArgumentsPropertyEffect::Assign(value.clone()),
                );
                self.push_local_get(temp_local);
                return Ok(true);
            }
            if let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(object)
            {
                self.emit_numeric_expression(object)?;
                self.state.emission.output.instructions.push(0x1a);
                self.emit_numeric_expression(property)?;
                self.state.emission.output.instructions.push(0x1a);
                let temp_local = self.allocate_temp_local();
                self.emit_numeric_expression(value)?;
                self.push_local_set(temp_local);
                if property_name == "callee" && arguments_binding.strict {
                    self.push_local_get(temp_local);
                    self.state.emission.output.instructions.push(0x1a);
                    return self.emit_error_throw().map(|_| true);
                }
                self.update_named_arguments_binding_effect(
                    object,
                    property_name,
                    ArgumentsPropertyEffect::Assign(value.clone()),
                );
                self.push_local_get(temp_local);
                return Ok(true);
            }
        }

        if self.is_direct_arguments_object(object) {
            if let Some(index) = argument_index_from_expression(property) {
                self.emit_arguments_slot_write(index, value)?;
                return Ok(true);
            }
            self.emit_dynamic_direct_arguments_property_write(property, value)?;
            return Ok(true);
        }

        if self.is_restricted_arrow_function_property(object, property) {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(property)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(value)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_named_error_throw("TypeError")?;
            return Ok(true);
        }

        Ok(false)
    }
}
