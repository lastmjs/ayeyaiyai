use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_store_identifier_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<()> {
        let resolved_local_binding = self.resolve_current_local_binding(name);
        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(value_local);
            self.push_local_set(parameter_scope_arguments_local);
        }
        if let Some((_, local_index)) = resolved_local_binding {
            self.push_local_get(value_local);
            self.push_local_set(local_index);
        } else if let Some(global_index) = self.backend.global_binding_index(name) {
            self.push_local_get(value_local);
            self.push_global_set(global_index);
        } else if self.emit_store_user_function_capture_binding_from_local(name, value_local)? {
        } else if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
        } else if let Some(binding) = self.backend.implicit_global_binding(name) {
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        } else {
            let binding = self.ensure_implicit_global_binding(name);
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_sync_identifier_runtime_value_from_local(
        &mut self,
        name: &str,
        value_local: u32,
    ) -> DirectResult<()> {
        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(value_local);
            self.push_local_set(parameter_scope_arguments_local);
        }
        if let Some((_, local_index)) = self.resolve_current_local_binding(name) {
            self.push_local_get(value_local);
            self.push_local_set(local_index);
        } else if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) {
            self.sync_user_function_capture_static_metadata(name, &hidden_name);
            self.emit_store_user_function_capture_binding_from_local(name, value_local)?;
        } else if self.emit_store_eval_local_function_binding_from_local(name, value_local)? {
        } else if let Some(global_index) = self.backend.global_binding_index(name) {
            self.push_local_get(value_local);
            self.push_global_set(global_index);
        } else if let Some(binding) = self.backend.implicit_global_binding(name) {
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        } else {
            let binding = self.ensure_implicit_global_binding(name);
            self.emit_store_implicit_global_from_local(binding, value_local)?;
        }
        Ok(())
    }
}
