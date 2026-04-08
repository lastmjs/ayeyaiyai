use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_plain_identifier_read_fallback(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        if self.emit_eval_lexical_binding_read(name)? {
            return Ok(());
        }
        if self.emit_parameter_default_binding_read(name)? {
            return Ok(());
        }
        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(parameter_scope_arguments_local);
        } else if parse_test262_realm_identifier(name).is_some()
            || parse_test262_realm_global_identifier(name).is_some()
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        } else if self.is_current_arguments_binding_name(name) && self.has_arguments_object() {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        } else if let Some((_, local_index)) = self.resolve_current_local_binding(name) {
            self.push_local_get(local_index);
        } else if let Some(function_binding) = self
            .state
            .speculation
            .static_semantics
            .local_function_binding(name)
            .cloned()
        {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(runtime_value) = self.user_function_runtime_value(&function_name) {
                        self.emit_prepare_user_function_capture_globals(&function_name)?;
                        self.push_i32_const(runtime_value);
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    self.push_i32_const(
                        builtin_function_runtime_value(&function_name)
                            .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                    );
                }
            }
        } else if let Some(global_index) = self.resolve_global_binding_index(name) {
            self.push_global_get(global_index);
        } else if self.emit_user_function_capture_binding_read(name)? {
        } else if self.emit_eval_local_function_binding_read(name)? {
        } else if name == "NaN" && self.is_unshadowed_builtin_identifier(name) {
            self.push_i32_const(JS_NAN_TAG);
        } else if name == "undefined" {
            self.push_i32_const(JS_UNDEFINED_TAG);
        } else if let Some(runtime_value) = builtin_function_runtime_value(name) {
            self.push_i32_const(runtime_value);
        } else if is_internal_user_function_identifier(name)
            && let Some(runtime_value) = self.user_function_runtime_value(name)
        {
            self.emit_prepare_user_function_capture_globals(name)?;
            self.push_i32_const(runtime_value);
        } else if let Some(kind) = self.lookup_identifier_kind(name) {
            let tag = kind.as_typeof_tag().unwrap_or(JS_UNDEFINED_TAG);
            self.push_i32_const(tag);
        } else {
            self.emit_print(&[Expression::String(format!(
                "missing identifier {name} in {:?}",
                self.state
                    .speculation
                    .execution_context
                    .current_user_function_name
            ))])?;
            self.emit_named_error_throw("ReferenceError")?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_plain_identifier_read(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        if self.parameter_scope_arguments_local_for(name).is_some()
            || (self.is_current_arguments_binding_name(name) && self.has_arguments_object())
            || self.resolve_current_local_binding(name).is_some()
            || self
                .state
                .speculation
                .static_semantics
                .has_local_function_binding(name)
            || self.resolve_global_binding_index(name).is_some()
            || self
                .resolve_user_function_capture_hidden_name(name)
                .is_some()
            || self.resolve_eval_local_function_hidden_name(name).is_some()
        {
            return self.emit_plain_identifier_read_fallback(name);
        }

        let Some(binding) = self.backend.implicit_global_binding(name) else {
            return self.emit_plain_identifier_read_fallback(name);
        };

        self.push_global_get(binding.present_index);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_global_get(binding.value_index);
        self.state.emission.output.instructions.push(0x05);
        self.emit_plain_identifier_read_fallback(name)?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
