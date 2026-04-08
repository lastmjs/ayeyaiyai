use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_control_transfer_statement(
        &mut self,
        statement: &Statement,
    ) -> DirectResult<()> {
        match statement {
            Statement::Break { label } => {
                let target_index = if let Some(label) = label.as_ref() {
                    match self.find_labeled_break(label)? {
                        Some(index) => index,
                        None => return Ok(()),
                    }
                } else {
                    match self
                        .state
                        .emission
                        .control_flow
                        .break_stack
                        .len()
                        .checked_sub(1)
                    {
                        Some(index) => index,
                        None => return Ok(()),
                    }
                };

                for context_index in
                    (target_index..self.state.emission.control_flow.break_stack.len()).rev()
                {
                    let break_hook = self.break_hook_for_target(
                        self.state.emission.control_flow.break_stack[context_index].break_target,
                    )?;
                    if let Some(break_hook) = break_hook {
                        self.emit_numeric_expression(&break_hook)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }

                let break_target =
                    self.state.emission.control_flow.break_stack[target_index].break_target;
                self.push_br(self.relative_depth(break_target));
                Ok(())
            }
            Statement::Continue { label } => {
                if label.is_some() {
                    let label = label
                        .as_ref()
                        .expect("labeled continue branch should include label");
                    let target_index = match self.find_labeled_loop_index(label)? {
                        Some(index) => index,
                        None => return Ok(()),
                    };
                    if target_index == self.state.emission.control_flow.loop_stack.len() - 1 {
                        let (continue_target, break_target) = {
                            let Some(loop_context) =
                                self.state.emission.control_flow.loop_stack.last()
                            else {
                                return Ok(());
                            };
                            (loop_context.continue_target, loop_context.break_target)
                        };
                        let break_hook = self.break_hook_for_target(break_target)?;
                        if let Some(break_hook) = break_hook {
                            self.emit_numeric_expression(&break_hook)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                        self.push_br(self.relative_depth(continue_target));
                        return Ok(());
                    }

                    for loop_index in
                        (target_index + 1..self.state.emission.control_flow.loop_stack.len()).rev()
                    {
                        let break_target =
                            self.state.emission.control_flow.loop_stack[loop_index].break_target;
                        if let Some(break_hook) = self.break_hook_for_target(break_target)? {
                            self.emit_numeric_expression(&break_hook)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }

                    let target =
                        self.state.emission.control_flow.loop_stack[target_index].continue_target;
                    self.push_br(self.relative_depth(target));
                    return Ok(());
                }
                let Some(loop_context) = self.state.emission.control_flow.loop_stack.last() else {
                    return Ok(());
                };
                let (continue_target, break_target) =
                    (loop_context.continue_target, loop_context.break_target);
                let break_hook = self.break_hook_for_target(break_target)?;
                if let Some(break_hook) = break_hook {
                    self.emit_numeric_expression(&break_hook)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
                self.push_br(self.relative_depth(continue_target));
                Ok(())
            }
            Statement::Return(expression) => {
                if !self.state.runtime.behavior.allow_return {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
                self.emit_numeric_expression(expression)?;
                self.clear_local_throw_state();
                self.clear_global_throw_state();
                self.state.emission.output.instructions.push(0x0f);
                Ok(())
            }
            Statement::Throw(expression) => {
                self.emit_numeric_expression(expression)?;
                self.push_local_set(self.state.runtime.throws.throw_value_local);
                self.push_i32_const(1);
                self.push_local_set(self.state.runtime.throws.throw_tag_local);
                self.emit_throw_from_locals()
            }
            Statement::Yield { value } => {
                self.emit_numeric_expression(value)?;
                self.state.emission.output.instructions.push(0x00);
                Ok(())
            }
            Statement::YieldDelegate { value } => {
                self.emit_numeric_expression(value)?;
                self.state.emission.output.instructions.push(0x00);
                Ok(())
            }
            _ => unreachable!("emit_control_transfer_statement called with non-control statement"),
        }
    }
}
