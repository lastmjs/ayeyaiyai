use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_expression_statement(&mut self, statement: &Statement) -> DirectResult<()> {
        match statement {
            Statement::Expression(expression) => {
                if self.emit_assert_throws_statement(expression)? {
                    return Ok(());
                }
                if self.has_current_user_function()
                    && let Expression::Call { callee, .. } = expression
                    && let Expression::Member { object, property } = callee.as_ref()
                    && matches!(
                        property.as_ref(),
                        Expression::String(name) if name == "then" || name == "catch"
                    )
                    && let Expression::Call {
                        callee: inner_callee,
                        ..
                    } = object.as_ref()
                    && let Expression::Member {
                        property: inner_property,
                        ..
                    } = inner_callee.as_ref()
                    && matches!(
                        inner_property.as_ref(),
                        Expression::String(name)
                            if matches!(name.as_str(), "then" | "catch" | "next" | "return" | "throw")
                    )
                {
                    self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                    self.state.emission.output.instructions.push(0x1a);
                    return Ok(());
                }
                if let Expression::Call { callee, arguments } = expression
                    && arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && matches!(property.as_ref(), Expression::String(name) if name == "next")
                    && matches!(object.as_ref(), Expression::Identifier(name) if self.state.speculation.static_semantics.has_local_array_iterator_binding(name))
                {
                    let hidden_name = self.allocate_named_hidden_local(
                        "direct_iterator_step_stmt",
                        StaticValueKind::Object,
                    );
                    self.update_local_iterator_step_binding(&hidden_name, expression);
                    self.emit_numeric_expression(object)?;
                    self.state.emission.output.instructions.push(0x1a);
                    self.update_member_function_binding_from_expression(expression);
                    self.update_object_binding_from_expression(expression);
                    return Ok(());
                }
                if let Expression::Call { callee, arguments } = expression
                    && let Expression::Member { object, property } = callee.as_ref()
                    && matches!(property.as_ref(), Expression::String(name) if name == "next")
                    && self.emit_fresh_simple_generator_next_call(object, arguments)?
                {
                    self.update_member_function_binding_from_expression(expression);
                    self.update_object_binding_from_expression(expression);
                    self.state.emission.output.instructions.push(0x1a);
                    return Ok(());
                }
                self.emit_numeric_expression(expression)?;
                self.update_member_function_binding_from_expression(expression);
                self.update_object_binding_from_expression(expression);
                self.state.emission.output.instructions.push(0x1a);
                Ok(())
            }
            Statement::Print { values } => self.emit_print(values),
            _ => unreachable!("emit_expression_statement called with non-expression statement"),
        }
    }
}
