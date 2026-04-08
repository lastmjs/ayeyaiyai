use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_typeof_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        if let Expression::Identifier(name) = expression
            && self
                .state
                .speculation
                .static_semantics
                .eval_lexical_initialized_locals
                .contains_key(name)
        {
            self.emit_eval_lexical_binding_read(name)?;
            self.state.emission.output.instructions.push(0x1a);
        }
        if let Expression::Identifier(name) = expression
            && self.resolve_current_local_binding(name).is_none()
            && !self
                .backend
                .global_semantics
                .names
                .bindings
                .contains_key(name)
            && self.emit_typeof_user_function_capture_binding(name)?
        {
            return Ok(());
        }
        if let Expression::Identifier(name) = expression
            && self.resolve_current_local_binding(name).is_none()
            && !self
                .backend
                .global_semantics
                .names
                .bindings
                .contains_key(name)
            && self.emit_typeof_eval_local_function_binding(name)?
        {
            return Ok(());
        }
        if self
            .resolve_function_binding_from_expression(expression)
            .is_some()
        {
            self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
            return Ok(());
        }
        if let Some(strict) = self.resolve_arguments_callee_strictness(expression) {
            if strict {
                return self.emit_error_throw();
            }
            self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
            return Ok(());
        }
        if let Expression::Identifier(name) = expression
            && self.is_identifier_bound(name)
        {
            self.emit_runtime_typeof_tag(expression)?;
            return Ok(());
        }
        let Some(type_tag) = self
            .infer_typeof_operand_kind(expression)
            .and_then(StaticValueKind::as_typeof_tag)
        else {
            self.emit_runtime_typeof_tag(expression)?;
            return Ok(());
        };
        self.push_i32_const(type_tag);
        Ok(())
    }
}
