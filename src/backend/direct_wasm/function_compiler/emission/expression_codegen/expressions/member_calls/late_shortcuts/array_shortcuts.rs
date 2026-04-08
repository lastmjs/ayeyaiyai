use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_array_member_call_shortcuts(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if matches!(property, Expression::String(property_name) if property_name == "push")
            && self.emit_tracked_array_push_call(object, arguments)?
        {
            return Ok(true);
        }
        if matches!(property, Expression::String(property_name) if property_name == "pop")
            && let Expression::Identifier(name) = object
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            let length_local = self
                .state
                .speculation
                .static_semantics
                .runtime_array_length_local(name);
            let use_global_runtime_array = self.is_named_global_array_binding(name)
                && (!self.state.speculation.execution_context.top_level_function
                    || self.uses_global_runtime_array_state(name));
            let mut popped_value = None;
            let mut popped_index = None;
            let mut new_length = None;
            if let Some(array_binding) = self
                .state
                .speculation
                .static_semantics
                .local_array_binding_mut(name)
            {
                popped_index = array_binding
                    .values
                    .len()
                    .checked_sub(1)
                    .map(|index| index as u32);
                popped_value = Some(
                    array_binding
                        .values
                        .pop()
                        .flatten()
                        .unwrap_or(Expression::Undefined),
                );
                new_length = Some(array_binding.values.len() as i32);
            } else if let Some(array_binding) = self
                .backend
                .global_semantics
                .values
                .array_bindings
                .get_mut(name)
            {
                popped_index = array_binding
                    .values
                    .len()
                    .checked_sub(1)
                    .map(|index| index as u32);
                popped_value = Some(
                    array_binding
                        .values
                        .pop()
                        .flatten()
                        .unwrap_or(Expression::Undefined),
                );
                new_length = Some(array_binding.values.len() as i32);
            }
            if let Some(popped_index) = popped_index {
                if use_global_runtime_array {
                    self.clear_global_runtime_array_slot(name, popped_index);
                } else {
                    self.clear_runtime_array_slot(name, popped_index);
                }
            }
            if let Some(new_length) = new_length {
                if !use_global_runtime_array && let Some(length_local) = length_local {
                    self.push_i32_const(new_length);
                    self.push_local_set(length_local);
                }
                if use_global_runtime_array {
                    self.emit_global_runtime_array_length_write(name, new_length);
                }
                self.emit_numeric_expression(
                    &popped_value.expect("tracked pop value should exist"),
                )?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}
