use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_logical_and(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        if inline_summary_side_effect_free_expression(left)
            && let Some(result) =
                self.resolve_static_logical_result_expression(BinaryOp::LogicalAnd, left, right)
        {
            return self.emit_numeric_expression(&result);
        }
        let temp_local = self.allocate_temp_local();
        self.emit_numeric_expression(left)?;
        self.push_local_set(temp_local);
        self.push_local_get(temp_local);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.emit_numeric_expression(right)?;
        self.state.emission.output.instructions.push(0x05);
        self.push_local_get(temp_local);
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.invalidate_operator_rhs_binding_metadata(right);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_logical_or(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        if inline_summary_side_effect_free_expression(left)
            && let Some(result) =
                self.resolve_static_logical_result_expression(BinaryOp::LogicalOr, left, right)
        {
            return self.emit_numeric_expression(&result);
        }
        let temp_local = self.allocate_temp_local();
        self.emit_numeric_expression(left)?;
        self.push_local_set(temp_local);
        self.push_local_get(temp_local);
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(temp_local);
        self.state.emission.output.instructions.push(0x05);
        self.emit_numeric_expression(right)?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.invalidate_operator_rhs_binding_metadata(right);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_nullish_coalescing(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        if inline_summary_side_effect_free_expression(left)
            && let Some(result) = self.resolve_static_logical_result_expression(
                BinaryOp::NullishCoalescing,
                left,
                right,
            )
        {
            return self.emit_numeric_expression(&result);
        }
        let temp_local = self.allocate_temp_local();

        self.emit_numeric_expression(left)?;
        self.push_local_set(temp_local);

        self.push_local_get(temp_local);
        self.push_i32_const(JS_NULL_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;

        self.push_local_get(temp_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x71);

        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();

        self.push_local_get(temp_local);

        self.state.emission.output.instructions.push(0x05);
        self.emit_numeric_expression(right)?;

        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();
        self.invalidate_operator_rhs_binding_metadata(right);
        Ok(())
    }

    fn invalidate_operator_rhs_binding_metadata(&mut self, expression: &Expression) {
        let mut invalidated_bindings = HashSet::new();
        collect_assigned_binding_names_from_expression(expression, &mut invalidated_bindings);
        let preserved_kinds = invalidated_bindings
            .iter()
            .filter_map(|name| {
                self.lookup_identifier_kind(name)
                    .map(|kind| (name.clone(), kind))
            })
            .collect::<HashMap<_, _>>();
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &invalidated_bindings,
            &preserved_kinds,
        );
    }
}
