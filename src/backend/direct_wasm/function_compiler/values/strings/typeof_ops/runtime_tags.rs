use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_tag(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        let value_local = self.allocate_temp_local();
        self.emit_numeric_expression(expression)?;
        self.push_local_set(value_local);
        self.emit_runtime_typeof_tag_from_local(value_local)
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_tag_from_local(
        &mut self,
        value_local: u32,
    ) -> DirectResult<()> {
        let result_local = self.allocate_temp_local();
        self.push_i32_const(JS_TYPEOF_NUMBER_TAG);
        self.push_local_set(result_local);

        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_NULL_TAG,
            JS_TYPEOF_OBJECT_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_UNDEFINED_TAG,
            JS_TYPEOF_UNDEFINED_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_STRING_TAG,
            JS_TYPEOF_STRING_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_BOOLEAN_TAG,
            JS_TYPEOF_BOOLEAN_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_OBJECT_TAG,
            JS_TYPEOF_OBJECT_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_UNDEFINED_TAG,
            JS_TYPEOF_UNDEFINED_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_FUNCTION_TAG,
            JS_TYPEOF_FUNCTION_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_BUILTIN_EVAL_VALUE,
            JS_TYPEOF_FUNCTION_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_SYMBOL_TAG,
            JS_TYPEOF_SYMBOL_TAG,
        )?;
        self.emit_runtime_typeof_exact_match(
            value_local,
            result_local,
            JS_TYPEOF_BIGINT_TAG,
            JS_TYPEOF_BIGINT_TAG,
        )?;
        self.emit_runtime_typeof_range_match(
            value_local,
            result_local,
            JS_NATIVE_ERROR_VALUE_BASE,
            JS_NATIVE_ERROR_VALUE_BASE + JS_NATIVE_ERROR_VALUE_LIMIT,
            JS_TYPEOF_OBJECT_TAG,
        )?;
        self.emit_runtime_typeof_range_match(
            value_local,
            result_local,
            JS_USER_FUNCTION_VALUE_BASE,
            JS_USER_FUNCTION_VALUE_BASE + JS_USER_FUNCTION_VALUE_LIMIT,
            JS_TYPEOF_FUNCTION_TAG,
        )?;

        self.push_local_get(result_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_exact_match(
        &mut self,
        value_local: u32,
        result_local: u32,
        match_value: i32,
        result_tag: i32,
    ) -> DirectResult<()> {
        self.push_local_get(value_local);
        self.push_i32_const(match_value);
        self.push_binary_op(BinaryOp::Equal)?;
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(result_tag);
        self.push_local_set(result_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_typeof_range_match(
        &mut self,
        value_local: u32,
        result_local: u32,
        start_inclusive: i32,
        end_exclusive: i32,
        result_tag: i32,
    ) -> DirectResult<()> {
        self.push_local_get(value_local);
        self.push_i32_const(start_inclusive);
        self.push_binary_op(BinaryOp::GreaterThanOrEqual)?;
        self.push_local_get(value_local);
        self.push_i32_const(end_exclusive);
        self.push_binary_op(BinaryOp::LessThan)?;
        self.state.emission.output.instructions.push(0x71);
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.push_i32_const(result_tag);
        self.push_local_set(result_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
