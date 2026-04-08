use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_runtime_string_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Expression::String(text) = object else {
            return Ok(false);
        };
        if let Some(index) = argument_index_from_expression(property) {
            if let Some(character) = text.chars().nth(index as usize) {
                self.emit_numeric_expression(&Expression::String(character.to_string()))?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(true);
        }
        if matches!(property, Expression::String(name) if name == "length") {
            self.push_i32_const(text.chars().count() as i32);
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn emit_runtime_arguments_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        if let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                if !arguments_binding.length_present {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                } else {
                    self.emit_numeric_expression(&arguments_binding.length_value)?;
                }
                return Ok(true);
            }
            if matches!(property, Expression::String(property_name) if property_name == "callee") {
                if arguments_binding.strict {
                    return self.emit_error_throw().map(|()| true);
                }
                if !arguments_binding.callee_present {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                } else if let Some(value) = arguments_binding.callee_value.as_ref() {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(true);
            }
            if let Some(index) = argument_index_from_expression(property) {
                if let Some(value) = arguments_binding.values.get(index as usize) {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(true);
            }
            self.emit_dynamic_arguments_binding_property_read(&arguments_binding, property)?;
            return Ok(true);
        }

        if self.is_direct_arguments_object(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                self.emit_direct_arguments_length()?;
                return Ok(true);
            }
            if matches!(property, Expression::String(text) if text == "callee") {
                self.emit_direct_arguments_callee()?;
                return Ok(true);
            }
            if let Some(index) = argument_index_from_expression(property) {
                self.emit_arguments_slot_read(index)?;
                return Ok(true);
            }
            self.emit_dynamic_direct_arguments_property_read(property)?;
            return Ok(true);
        }

        Ok(false)
    }

    pub(super) fn emit_runtime_returned_or_function_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        if let Some(returned_value) =
            self.resolve_returned_member_value_from_expression(object, property)
        {
            self.emit_numeric_expression(&returned_value)?;
            return Ok(true);
        }
        if self.emit_runtime_user_function_property_read(object, property)? {
            return Ok(true);
        }
        if matches!(property, Expression::String(text) if text == "constructor") {
            if let Some(binding) = self.resolve_constructed_object_constructor_binding(object) {
                match binding {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) = self.user_function(&function_name) {
                            self.push_i32_const(user_function_runtime_value(user_function));
                        } else {
                            self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        self.push_i32_const(
                            builtin_function_runtime_value(&function_name)
                                .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                        );
                    }
                }
            } else {
                self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
            }
            return Ok(true);
        }
        Ok(false)
    }
}
