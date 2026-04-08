use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_typeof_print(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        let Some(text) = self
            .infer_typeof_operand_kind(expression)
            .and_then(|kind| kind.as_typeof_str())
        else {
            let type_tag_local = self.allocate_temp_local();
            self.emit_runtime_typeof_tag(expression)?;
            self.push_local_set(type_tag_local);
            return self.emit_typeof_print_from_local(type_tag_local);
        };
        self.emit_print_string(text)
    }

    pub(in crate::backend::direct_wasm) fn emit_typeof_print_from_local(
        &mut self,
        type_tag_local: u32,
    ) -> DirectResult<()> {
        let done_local = self.allocate_temp_local();
        self.push_i32_const(0);
        self.push_local_set(done_local);

        for (type_tag, text) in [
            (JS_TYPEOF_BOOLEAN_TAG, "boolean"),
            (JS_TYPEOF_STRING_TAG, "string"),
            (JS_TYPEOF_OBJECT_TAG, "object"),
            (JS_TYPEOF_UNDEFINED_TAG, "undefined"),
            (JS_TYPEOF_FUNCTION_TAG, "function"),
            (JS_TYPEOF_SYMBOL_TAG, "symbol"),
            (JS_TYPEOF_BIGINT_TAG, "bigint"),
        ] {
            self.push_local_get(type_tag_local);
            self.push_i32_const(type_tag);
            self.push_binary_op(BinaryOp::Equal)?;
            self.state.emission.output.instructions.push(0x04);
            self.state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            self.push_control_frame();
            self.emit_print_string(text)?;
            self.push_i32_const(1);
            self.push_local_set(done_local);
            self.state.emission.output.instructions.push(0x0b);
            self.pop_control_frame();
        }

        self.push_local_get(done_local);
        self.state.emission.output.instructions.push(0x45);
        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();
        self.emit_print_string("number")?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
