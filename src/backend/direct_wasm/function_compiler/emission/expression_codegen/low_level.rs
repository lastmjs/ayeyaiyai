use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_loose_number(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        match expression {
            Expression::Null => {
                self.push_i32_const(0);
                Ok(())
            }
            Expression::Undefined => {
                self.push_i32_const(0);
                Ok(())
            }
            Expression::String(text) => {
                match parse_string_to_loose_i32(text) {
                    Ok(parsed) => self.push_i32_const(parsed),
                    Err(Unsupported("string literal collides with reserved JS tag")) => {
                        return Err(Unsupported("string literal collides with reserved JS tag"));
                    }
                    Err(_) => {
                        self.emit_static_string_literal(text)?;
                    }
                }
                Ok(())
            }
            _ => self.emit_numeric_expression(expression),
        }
    }

    pub(in crate::backend::direct_wasm) fn find_labeled_loop_index(
        &self,
        label: &str,
    ) -> DirectResult<Option<usize>> {
        Ok(self
            .loop_stack
            .iter()
            .rposition(|loop_context| loop_context.labels.iter().any(|name| name == label)))
    }

    pub(in crate::backend::direct_wasm) fn break_hook_for_target(
        &self,
        break_target: usize,
    ) -> DirectResult<Option<Expression>> {
        for break_context in self.break_stack.iter().rev() {
            if break_context.break_target == break_target {
                return Ok(break_context.break_hook.clone());
            }
        }
        Ok(None)
    }

    pub(in crate::backend::direct_wasm) fn find_labeled_break(
        &self,
        label: &str,
    ) -> DirectResult<Option<usize>> {
        Ok(self
            .break_stack
            .iter()
            .rposition(|break_context| break_context.labels.iter().any(|name| name == label)))
    }

    pub(in crate::backend::direct_wasm) fn allocate_temp_local(&mut self) -> u32 {
        let local_index = self.next_local_index;
        self.next_local_index += 1;
        local_index
    }

    pub(in crate::backend::direct_wasm) fn push_control_frame(&mut self) -> usize {
        self.control_stack.push(());
        self.control_stack.len() - 1
    }

    pub(in crate::backend::direct_wasm) fn pop_control_frame(&mut self) {
        self.control_stack.pop();
    }

    pub(in crate::backend::direct_wasm) fn relative_depth(&self, target: usize) -> u32 {
        (self.control_stack.len() - 1 - target) as u32
    }

    pub(in crate::backend::direct_wasm) fn push_i32_const(&mut self, value: i32) {
        self.instructions.push(0x41);
        push_i32(&mut self.instructions, value);
    }

    pub(in crate::backend::direct_wasm) fn push_local_get(&mut self, local_index: u32) {
        self.instructions.push(0x20);
        push_u32(&mut self.instructions, local_index);
    }

    pub(in crate::backend::direct_wasm) fn push_local_set(&mut self, local_index: u32) {
        self.instructions.push(0x21);
        push_u32(&mut self.instructions, local_index);
    }

    pub(in crate::backend::direct_wasm) fn push_global_get(&mut self, global_index: u32) {
        self.instructions.push(0x23);
        push_u32(&mut self.instructions, global_index);
    }

    pub(in crate::backend::direct_wasm) fn push_global_set(&mut self, global_index: u32) {
        self.instructions.push(0x24);
        push_u32(&mut self.instructions, global_index);
    }

    pub(in crate::backend::direct_wasm) fn push_local_tee(&mut self, local_index: u32) {
        self.instructions.push(0x22);
        push_u32(&mut self.instructions, local_index);
    }

    pub(in crate::backend::direct_wasm) fn push_call(&mut self, function_index: u32) {
        self.instructions.push(0x10);
        push_u32(&mut self.instructions, function_index);
    }

    pub(in crate::backend::direct_wasm) fn push_br(&mut self, relative_depth: u32) {
        self.instructions.push(0x0c);
        push_u32(&mut self.instructions, relative_depth);
    }

    pub(in crate::backend::direct_wasm) fn push_br_if(&mut self, relative_depth: u32) {
        self.instructions.push(0x0d);
        push_u32(&mut self.instructions, relative_depth);
    }
}
