use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_runtime_object_binding_member_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Some(object_binding) = self.resolve_object_binding_from_expression(object) else {
            return Ok(false);
        };
        let resolved_object = self
            .resolve_bound_alias_expression(object)
            .filter(|resolved| !static_expression_matches(resolved, object));
        let materialized_object = self.materialize_static_expression(object);
        let resolved_property = self.resolve_property_key_expression(property).or_else(|| {
            self.resolve_bound_alias_expression(property)
                .filter(|resolved| !static_expression_matches(resolved, property))
        });
        let materialized_property = self.materialize_static_expression(property);

        let getter_binding = self
            .resolve_member_getter_binding(object, property)
            .or_else(|| {
                resolved_object
                    .as_ref()
                    .and_then(|resolved| self.resolve_member_getter_binding(resolved, property))
            })
            .or_else(|| {
                resolved_property
                    .as_ref()
                    .and_then(|resolved| self.resolve_member_getter_binding(object, resolved))
            })
            .or_else(|| {
                resolved_object.as_ref().and_then(|resolved_object| {
                    resolved_property.as_ref().and_then(|resolved_property| {
                        self.resolve_member_getter_binding(resolved_object, resolved_property)
                    })
                })
            })
            .or_else(|| {
                (!static_expression_matches(&materialized_object, object))
                    .then(|| self.resolve_member_getter_binding(&materialized_object, property))?
            })
            .or_else(|| {
                (!static_expression_matches(&materialized_property, property))
                    .then(|| self.resolve_member_getter_binding(object, &materialized_property))?
            })
            .or_else(|| {
                (!static_expression_matches(&materialized_object, object)
                    || !static_expression_matches(&materialized_property, property))
                .then(|| {
                    self.resolve_member_getter_binding(&materialized_object, &materialized_property)
                })?
            });

        if let Some(function_binding) = getter_binding {
            let capture_slots = self.resolve_member_function_capture_slots(object, property);
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    self.emit_member_getter_call_with_bound_this(
                        &function_name,
                        object,
                        capture_slots.as_ref(),
                    )?;
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    let callee = Expression::Identifier(function_name);
                    if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
            }
            return Ok(true);
        }

        if let Some(value) = self.resolve_object_binding_property_value(&object_binding, property) {
            self.emit_numeric_expression(&value)?;
        } else if matches!(property, Expression::String(text) if text == "constructor") {
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
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
        } else if self.emit_runtime_user_function_property_read(object, property)? {
            return Ok(true);
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(true)
    }
}
