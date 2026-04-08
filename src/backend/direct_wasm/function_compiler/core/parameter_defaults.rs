use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn initialize_parameter_defaults(
        &mut self,
    ) -> DirectResult<()> {
        self.state.parameters.in_parameter_default_initialization = true;
        for (index, default) in self
            .state
            .parameters
            .parameter_defaults
            .clone()
            .into_iter()
            .enumerate()
        {
            let Some(parameter_name) = self.state.parameters.parameter_names.get(index).cloned()
            else {
                continue;
            };
            let parameter_local = index as u32;
            let initialized_local = self
                .state
                .parameters
                .parameter_initialized_locals
                .get(&parameter_name)
                .copied();

            let Some(default) = default else {
                if let Some(initialized_local) = initialized_local {
                    self.push_i32_const(1);
                    self.push_local_set(initialized_local);
                }
                continue;
            };

            self.push_local_get(parameter_local);
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();

            let default_value_local = self.allocate_temp_local();
            self.emit_numeric_expression(&default)?;
            self.push_local_set(default_value_local);
            self.emit_store_identifier_value_local(&parameter_name, &default, default_value_local)?;
            if let Some(initialized_local) = initialized_local {
                self.push_i32_const(1);
                self.push_local_set(initialized_local);
            }

            self.state.emission.output.instructions.push(0x05);
            if let Some(initialized_local) = initialized_local {
                self.push_i32_const(1);
                self.push_local_set(initialized_local);
            }
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        self.state.parameters.in_parameter_default_initialization = false;

        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn parameter_scope_arguments_local_for(
        &self,
        name: &str,
    ) -> Option<u32> {
        self.state
            .parameters
            .in_parameter_default_initialization
            .then_some(name)
            .filter(|name| self.is_current_arguments_binding_name(name))
            .and(self.state.parameters.parameter_scope_arguments_local)
    }

    pub(in crate::backend::direct_wasm) fn is_current_arguments_binding_name(
        &self,
        name: &str,
    ) -> bool {
        name == "arguments"
            || scoped_binding_source_name(name)
                .is_some_and(|source_name| source_name == "arguments")
    }

    pub(in crate::backend::direct_wasm) fn emit_parameter_default_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        if !self.state.parameters.in_parameter_default_initialization {
            return Ok(false);
        }
        let Some((resolved_name, local_index)) = self.resolve_current_local_binding(name) else {
            return Ok(false);
        };
        let Some(initialized_local) = self
            .state
            .parameters
            .parameter_initialized_locals
            .get(&resolved_name)
            .copied()
        else {
            return Ok(false);
        };
        self.push_local_get(initialized_local);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(local_index);
        self.state.emission.output.instructions.push(0x05);
        self.emit_named_error_throw("ReferenceError")?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }
}
