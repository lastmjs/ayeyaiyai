use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_terminal_throw_value_from_try_body(
        &self,
        body: &[Statement],
    ) -> Option<Expression> {
        let [statement] = body else {
            return None;
        };
        match statement {
            Statement::Declaration { body } | Statement::Block { body } => {
                self.resolve_terminal_throw_value_from_try_body(body)
            }
            Statement::Throw(expression) => Some(expression.clone()),
            Statement::Expression(expression) => {
                self.resolve_terminal_expression_throw_value(expression)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_throw_from_locals(&mut self) -> DirectResult<()> {
        self.push_local_get(self.state.runtime.throws.throw_value_local);
        self.push_global_set(THROW_VALUE_GLOBAL_INDEX);
        self.push_local_get(self.state.runtime.throws.throw_tag_local);
        self.push_global_set(THROW_TAG_GLOBAL_INDEX);

        let Some(try_context) = self.state.emission.control_flow.try_stack.last() else {
            if self.state.runtime.behavior.allow_return {
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.clear_local_throw_state();
                self.state.emission.output.instructions.push(0x0f);
                return Ok(());
            }
            self.emit_uncaught_throw_report_from_locals()?;
            self.state.emission.output.instructions.push(0x00);
            return Ok(());
        };

        self.push_br(self.relative_depth(try_context.catch_target));
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_check_global_throw_for_user_call(
        &mut self,
    ) -> DirectResult<()> {
        self.push_global_get(THROW_TAG_GLOBAL_INDEX);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();

        self.push_global_get(THROW_VALUE_GLOBAL_INDEX);
        self.push_local_set(self.state.runtime.throws.throw_value_local);
        self.push_global_get(THROW_TAG_GLOBAL_INDEX);
        self.push_local_set(self.state.runtime.throws.throw_tag_local);

        let Some(catch_target) = self
            .state
            .emission
            .control_flow
            .try_stack
            .last()
            .map(|try_context| try_context.catch_target)
        else {
            if self.state.runtime.behavior.allow_return {
                self.push_i32_const(JS_UNDEFINED_TAG);
                self.state.emission.output.instructions.push(0x0f);
            } else {
                self.emit_uncaught_throw_report_from_locals()?;
                self.state.emission.output.instructions.push(0x00);
            }
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(());
        };

        self.clear_global_throw_state();
        self.push_br(self.relative_depth(catch_target));
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn clear_local_throw_state(&mut self) {
        self.push_i32_const(0);
        self.push_local_set(self.state.runtime.throws.throw_tag_local);
        self.push_i32_const(0);
        self.push_local_set(self.state.runtime.throws.throw_value_local);
    }

    pub(in crate::backend::direct_wasm) fn clear_global_throw_state(&mut self) {
        self.push_i32_const(0);
        self.push_global_set(THROW_TAG_GLOBAL_INDEX);
        self.push_i32_const(0);
        self.push_global_set(THROW_VALUE_GLOBAL_INDEX);
    }

    pub(in crate::backend::direct_wasm) fn emit_error_throw(&mut self) -> DirectResult<()> {
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        self.push_local_set(self.state.runtime.throws.throw_value_local);
        self.push_i32_const(1);
        self.push_local_set(self.state.runtime.throws.throw_tag_local);
        self.emit_throw_from_locals()
    }

    pub(in crate::backend::direct_wasm) fn emit_named_error_throw(
        &mut self,
        name: &str,
    ) -> DirectResult<()> {
        if let Some(value) = native_error_runtime_value(name) {
            self.push_i32_const(value);
            self.push_local_set(self.state.runtime.throws.throw_value_local);
            self.push_i32_const(1);
            self.push_local_set(self.state.runtime.throws.throw_tag_local);
            return self.emit_throw_from_locals();
        }

        self.emit_error_throw()
    }

    pub(in crate::backend::direct_wasm) fn emit_static_throw_value(
        &mut self,
        throw_value: &StaticThrowValue,
    ) -> DirectResult<()> {
        match throw_value {
            StaticThrowValue::Value(expression) => {
                self.emit_numeric_expression(expression)?;
                self.push_local_set(self.state.runtime.throws.throw_value_local);
                self.push_i32_const(1);
                self.push_local_set(self.state.runtime.throws.throw_tag_local);
                self.emit_throw_from_locals()
            }
            StaticThrowValue::NamedError(name) => self.emit_named_error_throw(name),
        }
    }

    pub(in crate::backend::direct_wasm) fn emit_static_eval_outcome(
        &mut self,
        outcome: &StaticEvalOutcome,
    ) -> DirectResult<()> {
        match outcome {
            StaticEvalOutcome::Value(expression) => self.emit_numeric_expression(expression),
            StaticEvalOutcome::Throw(throw_value) => self.emit_static_throw_value(throw_value),
        }
    }
}
