use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_home_object_name_for_function(
        &self,
        function_name: &str,
    ) -> Option<String> {
        if let Some(home_object_name) = self
            .user_function(function_name)?
            .home_object_binding
            .as_ref()
        {
            return Some(home_object_name.clone());
        }
        self.find_global_home_object_binding_name(function_name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_base_expression_with_context(
        &self,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let function_name = current_function_name?;
        let home_object_name = self.resolve_home_object_name_for_function(function_name)?;
        self.global_object_prototype_expression(&home_object_name)
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_runtime_prototype_binding_with_context(
        &self,
        current_function_name: Option<&str>,
    ) -> Option<(String, GlobalObjectRuntimePrototypeBinding)> {
        let function_name = current_function_name?;
        let home_object_name = self.resolve_home_object_name_for_function(function_name)?;
        let binding = self
            .global_runtime_prototype_binding(&home_object_name)?
            .clone();
        Some((home_object_name, binding))
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_super_property_value_from_base(
        &mut self,
        base: Option<&Expression>,
        property: &Expression,
    ) -> DirectResult<()> {
        let Some(base) = base else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        };
        if let Some(function_binding) = self.resolve_member_function_binding(base, property) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.user_function(&function_name) {
                        self.push_i32_const(user_function_runtime_value(user_function));
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(_) => {
                    self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                }
            }
            return Ok(());
        }
        if let Some(function_binding) = self.resolve_member_getter_binding(base, property) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    self.emit_member_getter_call_with_bound_this(&function_name, base, None)?;
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    let callee = Expression::Identifier(function_name);
                    if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
            }
            return Ok(());
        }
        let materialized_property = self.materialize_static_expression(property);
        if let Some(object_binding) = self.resolve_object_binding_from_expression(base)
            && let Some(value) =
                object_binding_lookup_value(&object_binding, &materialized_property).cloned()
        {
            self.emit_numeric_expression(&value)?;
            return Ok(());
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_super_member_read_via_runtime_prototype_binding(
        &mut self,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Some((_, binding)) =
            self.resolve_super_runtime_prototype_binding_with_context(self.current_function_name())
        else {
            return Ok(false);
        };
        let Some(global_index) = binding.global_index else {
            return Ok(false);
        };
        let resolved_property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        if !matches!(
            resolved_property,
            Expression::String(_) | Expression::Identifier(_) | Expression::Member { .. }
        ) {
            return Ok(false);
        }

        let state_local = self.allocate_temp_local();
        self.push_global_get(global_index);
        self.push_local_set(state_local);

        let mut open_frames = 0;
        for (variant_index, prototype) in binding.variants.iter().enumerate() {
            self.push_local_get(state_local);
            self.push_i32_const(variant_index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_runtime_super_property_value_from_base(
                prototype.as_ref(),
                &resolved_property,
            )?;
            self.state.emission.output.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_function_binding(
        &self,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_super_function_binding_with_context(property, self.current_function_name())
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_function_binding_with_context(
        &self,
        property: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        let base = self.resolve_super_base_expression_with_context(current_function_name)?;
        let materialized_property = self.materialize_static_expression(property);
        self.resolve_member_function_binding(&base, property)
            .or_else(|| {
                self.resolve_object_binding_from_expression(&base)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(&object_binding, &materialized_property)
                            .cloned()
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(&value))
            })
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_getter_binding(
        &self,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let base = self.resolve_super_base_expression_with_context(self.current_function_name())?;
        self.resolve_member_getter_binding(&base, property)
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_value_expression(
        &self,
        property: &Expression,
    ) -> Option<Expression> {
        let base = self.resolve_super_base_expression_with_context(self.current_function_name())?;
        let materialized_property = self.materialize_static_expression(property);
        self.resolve_object_binding_from_expression(&base)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &materialized_property).cloned()
            })
    }
}
