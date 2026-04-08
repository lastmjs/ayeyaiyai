use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_member_getter_call_with_bound_this(
        &mut self,
        function_name: &str,
        this_expression: &Expression,
        capture_slots: Option<&BTreeMap<String, String>>,
    ) -> DirectResult<()> {
        let Some(user_function) = self.user_function(function_name).cloned() else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        };
        if let Some(capture_slots) = capture_slots {
            return self
                .emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                    &user_function,
                    &[],
                    JS_UNDEFINED_TAG,
                    this_expression,
                    capture_slots,
                );
        }
        self.emit_user_function_call_with_new_target_and_this_expression(
            &user_function,
            &[],
            JS_UNDEFINED_TAG,
            this_expression,
        )
    }

    pub(super) fn emit_member_binding_read_without_prelude(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        if let Some(function_binding) = self.resolve_member_getter_binding(object, property) {
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
        if let Some(function_binding) = self.resolve_member_function_binding(object, property) {
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
            return Ok(true);
        }
        if matches!(property, Expression::String(property_name) if property_name == "caller") {
            if let Some(strict) = self.resolve_arguments_callee_strictness(object) {
                if strict {
                    return self.emit_error_throw().map(|()| true);
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                return Ok(true);
            }
        }
        if self.is_restricted_arrow_function_property(object, property) {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            return self.emit_named_error_throw("TypeError").map(|()| true);
        }
        Ok(false)
    }
}
