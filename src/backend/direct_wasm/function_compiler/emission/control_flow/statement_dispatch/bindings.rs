use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_binding_statement(&mut self, statement: &Statement) -> DirectResult<()> {
        match statement {
            Statement::Var { name, value } => {
                let value_local = self.allocate_temp_local();
                let scoped_target = self.resolve_with_scope_binding(name)?;
                self.emit_numeric_expression(value)?;
                self.push_local_set(value_local);
                if let Some(scope_object) = scoped_target {
                    self.emit_scoped_property_store_from_local(
                        &scope_object,
                        name,
                        value_local,
                        value,
                    )?;
                    self.state.emission.output.instructions.push(0x1a);
                } else {
                    self.emit_store_identifier_value_local(name, value, value_local)?;
                }
                self.update_member_function_binding_from_expression(value);
                self.update_object_binding_from_expression(value);
                Ok(())
            }
            Statement::Let { name, value, .. } => {
                let value_local = self.allocate_temp_local();
                self.emit_numeric_expression(value)?;
                self.push_local_set(value_local);
                self.emit_store_identifier_value_local(name, value, value_local)?;
                if let Some(initialized_local) = self
                    .state
                    .speculation
                    .static_semantics
                    .eval_lexical_initialized_locals
                    .get(name)
                    .copied()
                {
                    self.push_i32_const(1);
                    self.push_local_set(initialized_local);
                }
                self.update_member_function_binding_from_expression(value);
                self.update_object_binding_from_expression(value);
                Ok(())
            }
            Statement::Assign { name, value } => {
                if self.try_emit_destructuring_default_assign_statement(name, value)? {
                    return Ok(());
                }
                let scoped_target = self.resolve_with_scope_binding(name)?;
                self.emit_numeric_expression(value)?;
                if let Some(scope_object) = scoped_target {
                    let value_local = self.allocate_temp_local();
                    self.push_local_set(value_local);
                    self.emit_scoped_property_store_from_local(
                        &scope_object,
                        name,
                        value_local,
                        value,
                    )?;
                    self.state.emission.output.instructions.push(0x1a);
                } else {
                    let value_local = self.allocate_temp_local();
                    self.push_local_set(value_local);
                    self.emit_store_identifier_value_local(name, value, value_local)?;
                }
                self.update_member_function_binding_from_expression(value);
                self.update_object_binding_from_expression(value);
                Ok(())
            }
            Statement::AssignMember {
                object,
                property,
                value,
            } => {
                self.emit_numeric_expression(&Expression::AssignMember {
                    object: Box::new(object.clone()),
                    property: Box::new(property.clone()),
                    value: Box::new(value.clone()),
                })?;
                self.state.emission.output.instructions.push(0x1a);
                Ok(())
            }
            _ => unreachable!("emit_binding_statement called with non-binding statement"),
        }
    }
}
