use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_runtime_user_function_property_read(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Expression::String(property_name) = property else {
            return Ok(false);
        };

        let mut candidates = Vec::new();
        for user_function in self.user_functions() {
            let Some(value) =
                self.runtime_user_function_property_value(&user_function, property_name)
            else {
                continue;
            };
            candidates.push((user_function_runtime_value(&user_function), value));
        }
        if candidates.is_empty() {
            return Ok(false);
        }

        let object_local = self.allocate_temp_local();
        let result_local = self.allocate_temp_local();
        let matched_local = self.allocate_temp_local();
        self.emit_numeric_expression(object)?;
        self.push_local_set(object_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_local_set(result_local);
        self.push_i32_const(0);
        self.push_local_set(matched_local);

        for (runtime_value, value) in candidates {
            self.push_local_get(object_local);
            self.push_i32_const(runtime_value);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_numeric_expression(&value)?;
            self.push_local_set(result_local);
            self.push_i32_const(1);
            self.push_local_set(matched_local);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(matched_local);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(result_local);
        self.state.emission.output.instructions.push(0x05);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(true)
    }
}
