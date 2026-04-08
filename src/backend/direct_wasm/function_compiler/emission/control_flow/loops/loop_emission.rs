use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_while(
        &mut self,
        condition: &Expression,
        break_hook: Option<&Expression>,
        labels: &[String],
        body: &[Statement],
    ) -> DirectResult<()> {
        let invalidated_bindings = self
            .collect_loop_assigned_binding_names_with_effectful_iterators(
                condition, break_hook, body, None, None,
            );
        let preserved_kinds = self.preserved_binding_kinds_for_loop(
            &invalidated_bindings,
            condition,
            break_hook,
            body,
            None,
        );
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        self.state.emission.output.instructions.push(0x02);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();

        self.state.emission.output.instructions.push(0x03);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        let continue_target = self.push_control_frame();
        self.state
            .emission
            .control_flow
            .loop_stack
            .push(LoopContext {
                break_target,
                continue_target,
                labels: labels.to_vec(),
                assigned_bindings: invalidated_bindings.clone(),
            });
        self.state
            .emission
            .control_flow
            .break_stack
            .push(BreakContext {
                break_target,
                labels: labels.to_vec(),
                break_hook: break_hook.cloned(),
            });

        self.emit_numeric_expression(condition)?;
        self.state.emission.output.instructions.push(0x45);
        self.push_br_if(self.relative_depth(break_target));
        self.emit_statements(body)?;
        self.push_br(self.relative_depth(continue_target));

        self.state.emission.control_flow.loop_stack.pop();
        self.state.emission.control_flow.break_stack.pop();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_do_while(
        &mut self,
        condition: &Expression,
        break_hook: Option<&Expression>,
        labels: &[String],
        body: &[Statement],
    ) -> DirectResult<()> {
        let invalidated_bindings = self
            .collect_loop_assigned_binding_names_with_effectful_iterators(
                condition, break_hook, body, None, None,
            );
        let preserved_kinds = self.preserved_binding_kinds_for_loop(
            &invalidated_bindings,
            condition,
            break_hook,
            body,
            None,
        );
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        self.state.emission.output.instructions.push(0x02);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();

        self.state.emission.output.instructions.push(0x03);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        let loop_target = self.push_control_frame();

        self.state.emission.output.instructions.push(0x02);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        let continue_target = self.push_control_frame();
        self.state
            .emission
            .control_flow
            .loop_stack
            .push(LoopContext {
                break_target,
                continue_target,
                labels: labels.to_vec(),
                assigned_bindings: invalidated_bindings.clone(),
            });
        self.state
            .emission
            .control_flow
            .break_stack
            .push(BreakContext {
                break_target,
                labels: labels.to_vec(),
                break_hook: break_hook.cloned(),
            });

        self.emit_statements(body)?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();

        self.emit_numeric_expression(condition)?;
        self.state.emission.output.instructions.push(0x45);
        self.push_br_if(self.relative_depth(break_target));
        self.push_br(self.relative_depth(loop_target));

        self.state.emission.control_flow.loop_stack.pop();
        self.state.emission.control_flow.break_stack.pop();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_for(
        &mut self,
        labels: &[String],
        init: &[Statement],
        per_iteration_bindings: &[String],
        condition: Option<&Expression>,
        update: Option<&Expression>,
        break_hook: Option<&Expression>,
        body: &[Statement],
    ) -> DirectResult<()> {
        let fallback_condition = Expression::Bool(true);
        let invalidated_bindings = self
            .collect_loop_assigned_binding_names_with_effectful_iterators(
                condition.unwrap_or(&fallback_condition),
                break_hook,
                body,
                Some(init),
                update,
            );
        self.with_active_eval_lexical_scope(per_iteration_bindings.to_vec(), |compiler| {
            compiler.emit_statements(init)?;
            let preserved_kinds = compiler.preserved_binding_kinds_for_loop(
                &invalidated_bindings,
                condition.unwrap_or(&fallback_condition),
                break_hook,
                body,
                update,
            );
            compiler.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &invalidated_bindings,
                &preserved_kinds,
            );

            compiler.state.emission.output.instructions.push(0x02);
            compiler
                .state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            let break_target = compiler.push_control_frame();

            compiler.state.emission.output.instructions.push(0x03);
            compiler
                .state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            let loop_target = compiler.push_control_frame();

            if let Some(condition) = condition {
                compiler.emit_numeric_expression(condition)?;
                compiler.state.emission.output.instructions.push(0x45);
                compiler.push_br_if(compiler.relative_depth(break_target));
            }

            compiler.state.emission.output.instructions.push(0x02);
            compiler
                .state
                .emission
                .output
                .instructions
                .push(EMPTY_BLOCK_TYPE);
            let continue_target = compiler.push_control_frame();
            compiler
                .state
                .emission
                .control_flow
                .loop_stack
                .push(LoopContext {
                    break_target,
                    continue_target,
                    labels: labels.to_vec(),
                    assigned_bindings: invalidated_bindings.clone(),
                });
            compiler
                .state
                .emission
                .control_flow
                .break_stack
                .push(BreakContext {
                    break_target,
                    labels: labels.to_vec(),
                    break_hook: break_hook.cloned(),
                });

            compiler.emit_statements(body)?;
            compiler.state.emission.output.instructions.push(0x0b);
            compiler.pop_control_frame();

            if let Some(update) = update {
                compiler.emit_numeric_expression(update)?;
                compiler.state.emission.output.instructions.push(0x1a);
            }
            compiler.push_br(compiler.relative_depth(loop_target));

            compiler.state.emission.control_flow.loop_stack.pop();
            compiler.state.emission.control_flow.break_stack.pop();
            compiler.state.emission.output.instructions.push(0x0b);
            compiler.pop_control_frame();
            compiler.state.emission.output.instructions.push(0x0b);
            compiler.pop_control_frame();
            compiler.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &invalidated_bindings,
                &preserved_kinds,
            );
            Ok(())
        })
    }

    pub(in crate::backend::direct_wasm) fn emit_switch_case(
        &mut self,
        case: &crate::ir::hir::SwitchCase,
        active_local: u32,
        discriminant_local: u32,
    ) -> DirectResult<()> {
        if let Some(test) = &case.test {
            self.push_local_get(active_local);
            self.push_i32_const(1);
            self.state.emission.output.instructions.push(0x46);

            self.push_local_get(active_local);
            self.state.emission.output.instructions.push(0x45);
            self.push_local_get(discriminant_local);
            self.emit_numeric_expression(test)?;
            self.state.emission.output.instructions.push(0x46);
            self.state.emission.output.instructions.push(0x71);
            self.state.emission.output.instructions.push(0x72);
        } else {
            self.push_local_get(active_local);
            self.state.emission.output.instructions.push(0x45);
        }

        self.state.emission.output.instructions.push(0x04);
        self.state
            .emission
            .output
            .instructions
            .push(EMPTY_BLOCK_TYPE);
        self.push_control_frame();

        self.push_i32_const(1);
        self.push_local_set(active_local);
        for case_statement in &case.body {
            self.emit_statement(case_statement)?;
        }

        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }
}
