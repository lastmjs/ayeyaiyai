use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_call_expression_dispatch(
        &mut self,
        expression: &Expression,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<()> {
        self.state
            .speculation
            .static_semantics
            .last_bound_user_function_call = None;
        if let Some(number) = self.resolve_static_number_value(expression) {
            return self.emit_numeric_expression(&Expression::Number(number));
        }
        if let Expression::Member { object, property } = callee
            && matches!(property.as_ref(), Expression::String(property_name) if property_name == "next")
            && self.emit_fresh_simple_generator_next_call(object, arguments)?
        {
            return Ok(());
        }
        if arguments.is_empty()
            && let Expression::Member { object, property } = callee
            && let Expression::String(property_name) = property.as_ref()
            && let Some(outcome) = self.resolve_static_member_call_outcome_with_context(
                object,
                property_name,
                self.current_function_name(),
            )
        {
            return self.emit_static_eval_outcome(&outcome);
        }
        if let Expression::Member { object, property } = callee
            && self.emit_member_getter_returned_user_function_call(object, property, arguments)?
        {
            return Ok(());
        }
        if self.emit_specialized_callee_call(callee, arguments)? {
            return Ok(());
        }
        if self.emit_static_weakref_deref_call(callee, arguments)? {
            return Ok(());
        }
        if self.emit_function_prototype_bind_call(callee, arguments)? {
            return Ok(());
        }
        if let Expression::Member { object, property } = callee
            && self.emit_early_member_call_shortcuts(object, property, arguments)?
        {
            return Ok(());
        }
        if let Expression::Identifier(name) = callee {
            return self.emit_identifier_call_expression(expression, callee, name, arguments);
        }
        if self.emit_resolved_function_binding_call_expression(expression, callee, arguments)? {
            return Ok(());
        }
        if !matches!(callee, Expression::Member { .. })
            && self.emit_dynamic_user_function_call(callee, arguments)?
        {
            return Ok(());
        }
        if let Expression::Member { object, property } = callee
            && self
                .emit_late_member_call_shortcuts(expression, callee, object, property, arguments)?
        {
            return Ok(());
        }

        self.emit_numeric_expression(callee)?;
        self.state.emission.output.instructions.push(0x1a);
        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }
}
