use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_eval_local_function_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self.hidden_implicit_global_binding(&hidden_name) else {
            return Ok(false);
        };

        self.push_global_get(binding.present_index);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.state.emission.output.instructions.push(0x05);
        self.emit_print(&[Expression::String(format!("missing eval local {name}"))])?;
        self.emit_named_error_throw("ReferenceError")?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_capture_binding_read(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self.hidden_implicit_global_binding(&hidden_name) else {
            return Ok(false);
        };

        self.push_global_get(binding.present_index);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.state.emission.output.instructions.push(0x05);
        self.emit_print(&[Expression::String(format!("missing user capture {name}"))])?;
        self.emit_named_error_throw("ReferenceError")?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_store_user_function_capture_binding_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self.hidden_implicit_global_binding(&hidden_name) else {
            return Ok(false);
        };
        self.push_local_get(value_local);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_store_eval_local_function_binding_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self.hidden_implicit_global_binding(&hidden_name) else {
            return Ok(false);
        };
        self.push_local_get(value_local);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        self.push_global_set(binding.present_index);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_delete_eval_local_function_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self.hidden_implicit_global_binding(&hidden_name) else {
            return Ok(false);
        };
        self.push_i32_const(0);
        self.push_global_set(binding.present_index);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_global_set(binding.value_index);
        self.push_i32_const(1);
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_eval_local_function_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self.hidden_implicit_global_binding(&hidden_name) else {
            return Ok(false);
        };
        let value_local = self.allocate_temp_local();

        self.push_global_get(binding.present_index);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.push_local_set(value_local);
        self.emit_runtime_typeof_tag_from_local(value_local)?;
        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(JS_TYPEOF_UNDEFINED_TAG);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_user_function_capture_binding(
        &mut self,
        name: &str,
    ) -> DirectResult<bool> {
        let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) else {
            return Ok(false);
        };
        let Some(binding) = self.hidden_implicit_global_binding(&hidden_name) else {
            return Ok(false);
        };
        let value_local = self.allocate_temp_local();

        self.push_global_get(binding.present_index);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.push_local_set(value_local);
        self.emit_runtime_typeof_tag_from_local(value_local)?;
        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(JS_TYPEOF_UNDEFINED_TAG);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }
}
