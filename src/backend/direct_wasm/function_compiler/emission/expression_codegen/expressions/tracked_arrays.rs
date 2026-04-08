use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_tracked_array_push_call(
        &mut self,
        object: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Expression::Identifier(name) = object else {
            return Ok(false);
        };
        let binding_name = if self
            .state
            .speculation
            .static_semantics
            .has_local_array_binding(name)
            || self
                .backend
                .global_semantics
                .values
                .array_bindings
                .contains_key(name)
            || self
                .state
                .speculation
                .static_semantics
                .runtime_array_length_local(name)
                .is_some()
            || self
                .state
                .speculation
                .static_semantics
                .has_runtime_array_slots(name)
        {
            name.clone()
        } else if let Some(hidden_name) = self
            .resolve_user_function_capture_hidden_name(name)
            .filter(|hidden_name| {
                self.state
                    .speculation
                    .static_semantics
                    .has_local_array_binding(hidden_name)
                    || self
                        .backend
                        .global_semantics
                        .values
                        .array_bindings
                        .contains_key(hidden_name)
                    || self
                        .state
                        .speculation
                        .static_semantics
                        .runtime_array_length_local(hidden_name)
                        .is_some()
                    || self
                        .state
                        .speculation
                        .static_semantics
                        .has_runtime_array_slots(hidden_name)
            })
        {
            hidden_name
        } else {
            name.clone()
        };
        if !self
            .state
            .speculation
            .static_semantics
            .has_local_array_binding(&binding_name)
            && !self
                .backend
                .global_semantics
                .values
                .array_bindings
                .contains_key(&binding_name)
        {
            return Ok(false);
        }

        let expanded_arguments = self.expand_call_arguments(arguments);
        let materialized_arguments = expanded_arguments
            .iter()
            .map(|argument| self.materialize_static_expression(argument))
            .collect::<Vec<_>>();
        let use_global_runtime_array = self.is_named_global_array_binding(&binding_name)
            && (!self.state.speculation.execution_context.top_level_function
                || self.uses_global_runtime_array_state(&binding_name));
        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);
        let argument_locals = expanded_arguments
            .iter()
            .map(|argument| {
                let local = self.allocate_temp_local();
                self.emit_numeric_expression(argument)?;
                self.push_local_set(local);
                Ok(local)
            })
            .collect::<DirectResult<Vec<_>>>()?;
        for argument_local in &argument_locals {
            self.push_local_get(*argument_local);
            self.state.emission.output.instructions.push(0x1a);
        }
        let mut old_length = None;
        let mut new_length = None;
        if let Some(array_binding) = self
            .state
            .speculation
            .static_semantics
            .local_array_binding_mut(&binding_name)
        {
            old_length = Some(array_binding.values.len() as u32);
            array_binding
                .values
                .extend(materialized_arguments.into_iter().map(Some));
            new_length = Some(array_binding.values.len() as i32);
        } else if let Some(array_binding) = self
            .backend
            .global_semantics
            .values
            .array_bindings
            .get_mut(&binding_name)
        {
            old_length = Some(array_binding.values.len() as u32);
            array_binding
                .values
                .extend(materialized_arguments.into_iter().map(Some));
            new_length = Some(array_binding.values.len() as i32);
        }
        let mut used_runtime_push = false;
        if let Some(old_length) = old_length {
            for (offset, argument_local) in argument_locals.iter().enumerate() {
                if !use_global_runtime_array
                    && self.emit_runtime_array_push_from_local(
                        &binding_name,
                        *argument_local,
                        &expanded_arguments[offset],
                    )?
                {
                    used_runtime_push = true;
                    if offset + 1 < argument_locals.len() {
                        self.state.emission.output.instructions.push(0x1a);
                    }
                    continue;
                }
                self.update_tracked_array_specialized_function_value(
                    &binding_name,
                    old_length + offset as u32,
                    &expanded_arguments[offset],
                )?;
                if use_global_runtime_array {
                    if self.emit_global_runtime_array_slot_write_from_local(
                        &binding_name,
                        old_length + offset as u32,
                        *argument_local,
                    )? {
                        self.state.emission.output.instructions.push(0x1a);
                    }
                } else if self.emit_runtime_array_slot_write_from_local(
                    &binding_name,
                    old_length + offset as u32,
                    *argument_local,
                )? {
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        if used_runtime_push {
            return Ok(true);
        }
        let new_length = new_length.expect("tracked push length should exist");
        if !use_global_runtime_array
            && let Some(length_local) = self
                .state
                .speculation
                .static_semantics
                .runtime_array_length_local(&binding_name)
        {
            self.push_i32_const(new_length);
            self.push_local_set(length_local);
        }
        if use_global_runtime_array {
            self.emit_global_runtime_array_length_write(&binding_name, new_length);
        }
        self.push_i32_const(new_length);
        Ok(true)
    }
}
