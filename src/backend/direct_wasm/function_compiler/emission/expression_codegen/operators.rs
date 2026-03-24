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
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.emit_numeric_expression(right)?;
        self.instructions.push(0x05);
        self.push_local_get(temp_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();
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
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(temp_local);
        self.instructions.push(0x05);
        self.emit_numeric_expression(right)?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_exponentiate(
        &mut self,
        base: &Expression,
        exponent: &Expression,
    ) -> DirectResult<()> {
        let base_local = self.allocate_temp_local();
        let result_local = self.allocate_temp_local();
        let exponent_local = self.allocate_temp_local();

        self.emit_numeric_expression(base)?;
        self.push_local_set(base_local);

        if let Expression::Number(power) = exponent {
            let power = f64_to_i32(*power)?;
            if power < 0 {
                self.push_i32_const(0);
            } else {
                self.push_i32_const(power);
            }
        } else {
            self.emit_numeric_expression(exponent)?;
        }
        self.push_local_set(exponent_local);

        self.push_i32_const(1);
        self.push_local_set(result_local);

        self.instructions.push(0x02);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();

        self.instructions.push(0x03);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let loop_target = self.push_control_frame();

        self.push_local_get(exponent_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::LessThanOrEqual)?;
        self.push_br_if(self.relative_depth(break_target));

        self.push_local_get(result_local);
        self.push_local_get(base_local);
        self.instructions.push(0x6c);
        self.push_local_set(result_local);

        self.push_local_get(exponent_local);
        self.push_i32_const(1);
        self.instructions.push(0x6b);
        self.push_local_set(exponent_local);

        self.push_br(self.relative_depth(loop_target));
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.instructions.push(0x0b);
        self.pop_control_frame();

        self.push_local_get(result_local);
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
        self.instructions.push(0x71);

        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();

        self.push_local_get(temp_local);

        self.instructions.push(0x05);
        self.emit_numeric_expression(right)?;

        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn push_binary_op(
        &mut self,
        op: BinaryOp,
    ) -> DirectResult<()> {
        let opcode = match op {
            BinaryOp::Add => 0x6a,
            BinaryOp::Subtract => 0x6b,
            BinaryOp::Multiply => 0x6c,
            BinaryOp::Divide => 0x6d,
            BinaryOp::Modulo => 0x6f,
            BinaryOp::Equal => 0x46,
            BinaryOp::NotEqual => 0x47,
            BinaryOp::LessThan => 0x48,
            BinaryOp::GreaterThan => 0x4a,
            BinaryOp::LessThanOrEqual => 0x4c,
            BinaryOp::GreaterThanOrEqual => 0x4e,
            BinaryOp::BitwiseAnd => 0x71,
            BinaryOp::BitwiseOr => 0x72,
            BinaryOp::BitwiseXor => 0x73,
            BinaryOp::LeftShift => 0x74,
            BinaryOp::RightShift => 0x75,
            BinaryOp::UnsignedRightShift => 0x76,
            _ => {
                self.push_i32_const(0);
                return Ok(());
            }
        };
        self.instructions.push(opcode);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn lookup_local(&self, name: &str) -> DirectResult<u32> {
        Ok(self.locals.get(name).copied().unwrap_or(self.param_count))
    }

    pub(in crate::backend::direct_wasm) fn emit_loose_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        self.emit_loose_number(left)?;
        self.emit_loose_number(right)?;
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_in_expression(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        if let Some(array_binding) = self.resolve_array_binding_from_expression(right) {
            if matches!(left, Expression::String(property_name) if property_name == "length") {
                self.push_i32_const(1);
                return Ok(());
            }
            if let Some(index) = argument_index_from_expression(left) {
                self.push_i32_const(
                    if array_binding
                        .values
                        .get(index as usize)
                        .is_some_and(|value| value.is_some())
                    {
                        1
                    } else {
                        0
                    },
                );
                return Ok(());
            }
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(right) {
            let materialized_left = self.materialize_static_expression(left);
            self.push_i32_const(
                if object_binding_has_property(&object_binding, &materialized_left) {
                    1
                } else {
                    0
                },
            );
            return Ok(());
        }
        if let Expression::Identifier(name) = right {
            if let Expression::String(property_name) = left {
                let has_property = match name.as_str() {
                    "Number" => matches!(
                        property_name.as_str(),
                        "MAX_VALUE"
                            | "MIN_VALUE"
                            | "NaN"
                            | "POSITIVE_INFINITY"
                            | "NEGATIVE_INFINITY"
                    ),
                    _ => false,
                };
                if has_property {
                    self.push_i32_const(1);
                    return Ok(());
                }
            }
        }
        self.emit_numeric_expression(left)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(right)?;
        self.instructions.push(0x1a);
        self.push_i32_const(0);
        Ok(())
    }
}
