use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn clear_runtime_object_property_shadow_binding(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> bool {
        let Some(binding) = self.resolve_runtime_object_property_shadow_binding(object, property)
        else {
            return false;
        };
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_global_set(binding.value_index);
        self.push_i32_const(0);
        self.push_global_set(binding.present_index);
        true
    }

    pub(in crate::backend::direct_wasm) fn runtime_object_property_shadow_binding_name(
        owner_name: &str,
        property_name: &str,
    ) -> String {
        format!("__ayy_object_property__{owner_name}__{property_name}")
    }

    pub(in crate::backend::direct_wasm) fn runtime_object_property_shadow_owner_name_for_identifier(
        &self,
        name: &str,
    ) -> Option<String> {
        if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name) {
            return Some(hidden_name);
        }
        if let Some(hidden_name) = self.resolve_eval_local_function_hidden_name(name) {
            return Some(hidden_name);
        }
        if self
            .state
            .speculation
            .static_semantics
            .has_local_object_binding(name)
        {
            return Some(name.to_string());
        }
        ((self.backend.global_has_binding(name) || self.backend.global_has_implicit_binding(name))
            && self.backend.global_object_binding(name).is_some())
        .then(|| name.to_string())
        .or_else(|| {
            (self.backend.global_has_implicit_binding(name)
                && self.backend.global_object_binding(name).is_some())
            .then(|| name.to_string())
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_runtime_object_property_shadow_binding(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> Option<ImplicitGlobalBinding> {
        let Expression::Identifier(name) = object else {
            return None;
        };
        let property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        let Expression::String(property_name) = property else {
            return None;
        };
        let owner_name = self.runtime_object_property_shadow_owner_name_for_identifier(name)?;
        Some(self.ensure_implicit_global_binding(
            &Self::runtime_object_property_shadow_binding_name(&owner_name, &property_name),
        ))
    }

    pub(in crate::backend::direct_wasm) fn runtime_object_property_shadow_binding_by_names(
        &mut self,
        owner_name: &str,
        property_name: &str,
    ) -> ImplicitGlobalBinding {
        self.ensure_implicit_global_binding(&Self::runtime_object_property_shadow_binding_name(
            owner_name,
            property_name,
        ))
    }

    pub(in crate::backend::direct_wasm) fn object_runtime_shadow_properties(
        &self,
        owner_name: &str,
    ) -> Vec<(String, Expression)> {
        let object_expression = Expression::Identifier(owner_name.to_string());
        let Some(object_binding) = self.resolve_object_binding_from_expression(&object_expression)
        else {
            return Vec::new();
        };
        ordered_object_property_names(&object_binding)
            .into_iter()
            .filter_map(|property_name| {
                object_binding_lookup_value(
                    &object_binding,
                    &Expression::String(property_name.clone()),
                )
                .cloned()
                .map(|value| (property_name, value))
            })
            .collect()
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_shadow_fallback_value(
        &mut self,
        fallback_value: &Expression,
    ) -> DirectResult<()> {
        if let Some(function_binding) =
            self.resolve_function_binding_from_expression(fallback_value)
        {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.user_function(&function_name) {
                        self.push_i32_const(user_function_runtime_value(user_function));
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
            return Ok(());
        }

        if self
            .resolve_array_binding_from_expression(fallback_value)
            .is_some()
            || self
                .resolve_object_binding_from_expression(fallback_value)
                .is_some()
            || self
                .resolve_arguments_binding_from_expression(fallback_value)
                .is_some()
            || self
                .resolve_proxy_binding_from_expression(fallback_value)
                .is_some()
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }

        self.emit_numeric_expression(fallback_value)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_object_property_shadow_copy(
        &mut self,
        source_owner: &str,
        target_owner: &str,
    ) -> DirectResult<()> {
        if source_owner == target_owner {
            return Ok(());
        }
        for (property_name, fallback_value) in self.object_runtime_shadow_properties(source_owner) {
            let source_binding =
                self.runtime_object_property_shadow_binding_by_names(source_owner, &property_name);
            let target_binding =
                self.runtime_object_property_shadow_binding_by_names(target_owner, &property_name);
            self.push_global_get(source_binding.present_index);
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.push_global_get(source_binding.value_index);
            self.push_global_set(target_binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(target_binding.present_index);
            self.state.emission.output.instructions.push(0x05);
            self.emit_runtime_shadow_fallback_value(&fallback_value)?;
            self.push_global_set(target_binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(target_binding.present_index);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_object_spread_copy_data_properties_effects(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        if !inline_summary_side_effect_free_expression(expression) {
            return Ok(());
        }
        let Some(object_binding) = self.resolve_object_binding_from_expression(expression) else {
            return Ok(());
        };

        for property_name in ordered_object_property_names(&object_binding) {
            if object_binding
                .non_enumerable_string_properties
                .iter()
                .any(|hidden_name| hidden_name == &property_name)
            {
                continue;
            }
            self.emit_member_read_without_prelude(expression, &Expression::String(property_name))?;
            self.state.emission.output.instructions.push(0x1a);
        }
        for (property, _) in &object_binding.symbol_properties {
            self.emit_member_read_without_prelude(expression, property)?;
            self.state.emission.output.instructions.push(0x1a);
        }

        Ok(())
    }
}
