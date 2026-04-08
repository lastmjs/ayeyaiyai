use super::*;

impl<'a> FunctionCompiler<'a> {
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
        if let Expression::Identifier(name) = right
            && let Expression::String(property_name) = left
        {
            let has_property = match name.as_str() {
                "Number" => matches!(
                    property_name.as_str(),
                    "MAX_VALUE" | "MIN_VALUE" | "NaN" | "POSITIVE_INFINITY" | "NEGATIVE_INFINITY"
                ),
                _ => false,
            };
            if has_property {
                self.push_i32_const(1);
                return Ok(());
            }
        }
        self.emit_numeric_expression(left)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(right)?;
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(0);
        Ok(())
    }
}
