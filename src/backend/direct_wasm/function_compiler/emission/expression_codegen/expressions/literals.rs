use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_literal_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        match expression {
            Expression::Number(value) => {
                if value.is_nan() {
                    self.push_i32_const(JS_NAN_TAG);
                } else {
                    self.push_i32_const(f64_to_i32(*value)?);
                }
                Ok(())
            }
            Expression::BigInt(value) => {
                self.push_i32_const(parse_bigint_to_i32(value)?);
                Ok(())
            }
            Expression::String(text) => {
                match parse_string_to_i32(text) {
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
            Expression::Null => {
                self.push_i32_const(JS_NULL_TAG);
                Ok(())
            }
            Expression::Undefined => {
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            Expression::Bool(value) => {
                self.push_i32_const(if *value { 1 } else { 0 });
                Ok(())
            }
            Expression::Array(elements) => self.emit_array_literal_expression(elements),
            Expression::Object(entries) => self.emit_object_literal_expression(entries),
            _ => unreachable!("literal expression expected"),
        }
    }

    fn emit_array_literal_expression(
        &mut self,
        elements: &[crate::ir::hir::ArrayElement],
    ) -> DirectResult<()> {
        for element in elements {
            match element {
                crate::ir::hir::ArrayElement::Expression(expression)
                | crate::ir::hir::ArrayElement::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }

    fn emit_object_literal_expression(
        &mut self,
        entries: &[crate::ir::hir::ObjectEntry],
    ) -> DirectResult<()> {
        for entry in entries {
            match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    self.emit_property_key_expression_effects(key)?;
                    self.emit_numeric_expression(value)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                    self.emit_property_key_expression_effects(key)?;
                    self.emit_numeric_expression(getter)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
                crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                    self.emit_property_key_expression_effects(key)?;
                    self.emit_numeric_expression(setter)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
                crate::ir::hir::ObjectEntry::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                    self.emit_object_spread_copy_data_properties_effects(expression)?;
                }
            }
        }
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }
}
