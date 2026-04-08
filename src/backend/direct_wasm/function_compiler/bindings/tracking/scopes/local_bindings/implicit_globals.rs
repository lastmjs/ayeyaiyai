use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn clear_eval_local_function_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.state.clear_eval_local_function_binding_metadata(name);
    }

    pub(in crate::backend::direct_wasm) fn clear_static_identifier_binding_metadata(
        &mut self,
        name: &str,
    ) {
        self.state.clear_local_static_binding_metadata(name);

        self.clear_global_binding_state(name);
        self.backend
            .clear_global_object_literal_member_bindings_for_name(name);
    }

    pub(in crate::backend::direct_wasm) fn emit_delete_implicit_global_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(binding) = self.backend.implicit_global_binding(name) else {
            return Ok(false);
        };
        self.clear_static_identifier_binding_metadata(name);
        self.push_i32_const(0);
        self.push_global_set(binding.present_index);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_store_implicit_global_from_local(
        &mut self,
        binding: ImplicitGlobalBinding,
        value_local: u32,
    ) -> DirectResult<()> {
        if self.state.speculation.execution_context.strict_mode {
            self.push_global_get(binding.present_index);
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_local_get(value_local);
            self.push_global_set(binding.value_index);
            self.state.emission.output.instructions.push(0x05);
            self.emit_named_error_throw("ReferenceError")?;
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(());
        }

        self.push_local_get(value_local);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);
        Ok(())
    }
}
