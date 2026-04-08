use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_special_member_read_without_prelude(
        &mut self,
        object: &Expression,
        property: &Expression,
        static_array_property: &Expression,
    ) -> DirectResult<bool> {
        if matches!(property, Expression::String(property_name) if property_name == "global")
            && matches!(
                object,
                Expression::Call { callee, arguments }
                    if arguments.is_empty()
                        && matches!(
                            callee.as_ref(),
                            Expression::Member { object, property }
                                if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                                    && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
                        )
            )
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }
        if self.state.speculation.execution_context.top_level_function
            && matches!(object, Expression::This)
        {
            let property = self
                .resolve_property_key_expression(property)
                .unwrap_or_else(|| self.materialize_static_expression(property));
            if let Expression::String(property_name) = property {
                if let Some(state) = self.backend.global_property_descriptor(&property_name) {
                    if let Some(value) = state.writable.map(|_| state.value.clone()) {
                        self.emit_numeric_expression(&value)?;
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                    return Ok(true);
                }
                if property_name == "NaN" {
                    self.push_i32_const(JS_NAN_TAG);
                    return Ok(true);
                }
                if property_name == "undefined" {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                }
                if let Some(kind) = builtin_identifier_kind(&property_name) {
                    match kind {
                        StaticValueKind::Function => {
                            self.push_i32_const(
                                builtin_function_runtime_value(&property_name)
                                    .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                            );
                            return Ok(true);
                        }
                        StaticValueKind::Object => {
                            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                            return Ok(true);
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(text) = self.resolve_static_string_value(&Expression::Member {
            object: Box::new(object.clone()),
            property: Box::new(property.clone()),
        }) {
            self.emit_static_string_literal(&text)?;
            return Ok(true);
        }
        if matches!(property, Expression::String(property_name) if property_name == "length")
            && self
                .resolve_function_binding_from_expression(object)
                .is_none()
            && self
                .resolve_member_getter_binding(object, property)
                .is_none()
            && self
                .resolve_member_function_binding(object, property)
                .is_none()
            && self
                .resolve_member_setter_binding(object, property)
                .is_none()
            && let Expression::String(text) = self.materialize_static_expression(object)
        {
            self.push_i32_const(text.encode_utf16().count() as i32);
            return Ok(true);
        }
        if matches!(object, Expression::Identifier(name) if name == "Number" && self.is_unshadowed_builtin_identifier(name))
            && matches!(property, Expression::String(property_name) if property_name == "NaN")
        {
            self.push_i32_const(JS_NAN_TAG);
            return Ok(true);
        }
        if let Some(step_binding) = self.resolve_iterator_step_binding_from_expression(object) {
            if let Expression::String(property_name) = property {
                match property_name.as_str() {
                    "done" => {
                        match step_binding {
                            IteratorStepBinding::Runtime { done_local, .. } => {
                                self.push_local_get(done_local);
                            }
                        }
                        return Ok(true);
                    }
                    "value" => {
                        match step_binding {
                            IteratorStepBinding::Runtime { value_local, .. } => {
                                self.push_local_get(value_local);
                            }
                        }
                        return Ok(true);
                    }
                    _ => {}
                }
            }
        }
        if let Expression::Identifier(name) = object {
            if self
                .state
                .speculation
                .static_semantics
                .has_local_typed_array_view_binding(name)
            {
                if matches!(static_array_property, Expression::String(text) if text == "length") {
                    if let Some(length_local) = self
                        .state
                        .speculation
                        .static_semantics
                        .runtime_array_length_local(name)
                    {
                        self.push_local_get(length_local);
                    } else {
                        self.push_i32_const(0);
                    }
                    return Ok(true);
                }
                if let Some(index) = argument_index_from_expression(static_array_property) {
                    if let Some(oob_local) = self
                        .state
                        .speculation
                        .static_semantics
                        .runtime_typed_array_oob_local(name)
                    {
                        self.push_local_get(oob_local);
                        self.state.emission.output.instructions.push(0x04);
                        self.state.emission.output.instructions.push(I32_TYPE);
                        self.push_control_frame();
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        self.state.emission.output.instructions.push(0x05);
                        if !self.emit_runtime_array_slot_read(name, index)? {
                            self.push_i32_const(JS_UNDEFINED_TAG);
                        }
                        self.state.emission.output.instructions.push(0x0b);
                        self.pop_control_frame();
                    } else if !self.emit_runtime_array_slot_read(name, index)? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                    return Ok(true);
                }
            }
        }
        if let Some(bytes_per_element) =
            self.resolve_typed_array_builtin_bytes_per_element(object, property)
        {
            self.push_i32_const(bytes_per_element as i32);
            return Ok(true);
        }
        if let Some(function_name) = self.resolve_function_name_value(object, property) {
            self.emit_static_string_literal(&function_name)?;
            return Ok(true);
        }
        if let Some(function_length) = self.resolve_user_function_length(object, property) {
            self.push_i32_const(function_length as i32);
            return Ok(true);
        }
        Ok(false)
    }
}
