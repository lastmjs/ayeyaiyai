use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn is_direct_arguments_object(
        &self,
        expression: &Expression,
    ) -> bool {
        match expression {
            Expression::Identifier(name) if self.is_current_arguments_binding_name(name) => {
                self.has_arguments_object()
            }
            Expression::Identifier(name) => self
                .state
                .parameters
                .direct_arguments_aliases
                .contains(name),
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn direct_arguments_callee_expression(
        &self,
    ) -> Option<Expression> {
        self.state
            .speculation
            .execution_context
            .current_arguments_callee_override
            .clone()
            .or_else(|| {
                self.state
                    .speculation
                    .execution_context
                    .current_user_function_name
                    .as_ref()
                    .map(|name| Expression::Identifier(name.clone()))
            })
    }

    pub(in crate::backend::direct_wasm) fn direct_arguments_has_property(
        &self,
        property_name: &str,
    ) -> bool {
        match property_name {
            "callee" => {
                self.state
                    .speculation
                    .execution_context
                    .current_arguments_callee_present
            }
            "length" => {
                self.state
                    .speculation
                    .execution_context
                    .current_arguments_length_present
            }
            _ => false,
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_update_arguments_slot_mapping(
        &mut self,
        slot: &ArgumentsSlot,
    ) {
        if let Some(mapped_local) = slot.mapped_local {
            self.push_i32_const(if slot.state.mapped { 1 } else { 0 });
            self.push_local_set(mapped_local);
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_direct_arguments_length(
        &mut self,
    ) -> DirectResult<()> {
        if !self
            .state
            .speculation
            .execution_context
            .current_arguments_length_present
        {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        }
        if let Some(value) = self
            .state
            .speculation
            .execution_context
            .current_arguments_length_override
            .clone()
        {
            self.emit_numeric_expression(&value)?;
            return Ok(());
        }
        self.emit_arguments_length();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_direct_arguments_callee(
        &mut self,
    ) -> DirectResult<()> {
        if self.state.speculation.execution_context.strict_mode {
            return self.emit_error_throw();
        }
        if !self
            .state
            .speculation
            .execution_context
            .current_arguments_callee_present
        {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        }
        if let Some(value) = self.direct_arguments_callee_expression() {
            self.emit_numeric_expression(&value)?;
        } else {
            self.push_i32_const(JS_UNDEFINED_TAG);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_property_name_match(
        &mut self,
        property_local: u32,
        property_name: &str,
    ) -> DirectResult<()> {
        let (ptr, _) = self.intern_string(property_name.as_bytes().to_vec());
        self.push_local_get(property_local);
        self.push_i32_const(ptr as i32);
        self.push_binary_op(BinaryOp::Equal)
    }
}
