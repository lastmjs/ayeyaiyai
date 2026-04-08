use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn try_emit_destructuring_default_assign_statement(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<bool> {
        let Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } = value
        else {
            return Ok(false);
        };
        let Expression::Binary {
            op: BinaryOp::NotEqual,
            left,
            right,
        } = condition.as_ref()
        else {
            return Ok(false);
        };
        if !matches!(right.as_ref(), Expression::Undefined) {
            return Ok(false);
        }
        let Expression::Assign {
            name: temporary_name,
            value: temporary_value_expression,
        } = left.as_ref()
        else {
            return Ok(false);
        };
        let Expression::Identifier(then_name) = then_expression.as_ref() else {
            return Ok(false);
        };
        if then_name != temporary_name
            || !self
                .state
                .runtime
                .locals
                .bindings
                .contains_key(temporary_name)
        {
            return Ok(false);
        }
        let Expression::Member { object, property } = temporary_value_expression.as_ref() else {
            return Ok(false);
        };

        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);
        let resolved_property = self.emit_property_key_expression_effects(property)?;
        let effective_property = resolved_property.as_ref().unwrap_or(property.as_ref());

        let scoped_target = self.resolve_with_scope_binding(name)?;

        self.emit_member_read_without_prelude(object, effective_property)?;
        let temporary_local = self.lookup_local(temporary_name)?;
        self.push_local_set(temporary_local);

        self.push_local_get(temporary_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.state.emission.output.instructions.push(0x04);
        self.state.emission.output.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(temporary_local);
        self.state.emission.output.instructions.push(0x05);
        self.emit_numeric_expression(else_expression)?;
        self.state.emission.output.instructions.push(0x0b);
        self.pop_control_frame();

        let value_local = self.allocate_temp_local();
        self.push_local_set(value_local);
        if let Some(scope_object) = scoped_target {
            self.emit_scoped_property_store_from_local(&scope_object, name, value_local, value)?;
            self.state.emission.output.instructions.push(0x1a);
        } else {
            self.emit_store_identifier_value_local(name, value, value_local)?;
        }

        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn note_identifier_numeric_kind(&mut self, name: &str) {
        let names = HashSet::from([name.to_string()]);
        let preserved_kinds = HashMap::from([(name.to_string(), StaticValueKind::Number)]);
        self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
            &names,
            &preserved_kinds,
        );
    }
}
