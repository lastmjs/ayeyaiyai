use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_identifier_expression_value(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        if let Some(scope_object) = self.resolve_with_scope_binding(name)? {
            self.emit_scoped_property_read(&scope_object, name)?;
        } else {
            self.emit_plain_identifier_read(name)?;
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_assign_expression_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        let scoped_target = self.resolve_with_scope_binding(name)?;
        self.emit_numeric_expression(value)?;
        if let Some(scope_object) = scoped_target {
            let value_local = self.allocate_temp_local();
            self.push_local_set(value_local);
            self.emit_scoped_property_store_from_local(&scope_object, name, value_local, value)?;
        } else {
            let value_local = self.allocate_temp_local();
            self.push_local_set(value_local);
            self.emit_store_identifier_value_local(name, value, value_local)?;
            self.push_local_get(value_local);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_member_expression_value(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<()> {
        if self.emit_direct_iterator_step_member_read(object, property)? {
            return Ok(());
        }
        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);
        let resolved_property = self.emit_property_key_expression_effects(property)?;
        let effective_property = resolved_property.as_ref().unwrap_or(property);
        self.emit_member_read_without_prelude(object, effective_property)
    }

    pub(in crate::backend::direct_wasm) fn emit_super_member_expression_value(
        &mut self,
        property: &Expression,
    ) -> DirectResult<()> {
        if self.emit_super_member_read_via_runtime_prototype_binding(property)? {
            return Ok(());
        }
        if let Some(function_binding) = self.resolve_super_function_binding(property) {
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
        if let Some(function_binding) = self.resolve_super_getter_binding(property) {
            self.emit_numeric_expression(property)?;
            self.state.emission.output.instructions.push(0x1a);
            let callee = match function_binding {
                LocalFunctionBinding::User(function_name)
                | LocalFunctionBinding::Builtin(function_name) => {
                    Expression::Identifier(function_name)
                }
            };
            if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(());
        }
        if let Some(value) = self.resolve_super_value_expression(property) {
            self.emit_numeric_expression(&value)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_this_expression_value(
        &mut self,
    ) -> DirectResult<()> {
        if self.current_function_is_derived_constructor() {
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state.emission.output.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.emit_named_error_throw("ReferenceError")?;
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.state.emission.output.instructions.push(0x05);
            self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(());
        }
        self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
        Ok(())
    }
}
