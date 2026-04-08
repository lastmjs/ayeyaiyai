use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_string_member_call_shortcuts(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if matches!(property, Expression::String(property_name) if property_name == "indexOf")
            && let Expression::String(text) = object
            && let [CallArgument::Expression(Expression::String(search))] = arguments
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(&Expression::String(search.clone()))?;
            self.state.emission.output.instructions.push(0x1a);
            self.push_i32_const(text.find(search).map(|index| index as i32).unwrap_or(-1));
            return Ok(true);
        }
        if matches!(property, Expression::String(property_name) if property_name == "replace")
            && inline_summary_side_effect_free_expression(object)
            && let Some(source_text) = self.resolve_static_string_value(object)
            && let [
                CallArgument::Expression(search_expression),
                CallArgument::Expression(replacement_expression),
            ] = arguments
            && inline_summary_side_effect_free_expression(search_expression)
            && inline_summary_side_effect_free_expression(replacement_expression)
            && let Some(search_text) = self.resolve_static_string_value(search_expression)
        {
            self.emit_numeric_expression(object)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(search_expression)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_numeric_expression(replacement_expression)?;
            self.state.emission.output.instructions.push(0x1a);

            let replacement_text = if let Some(replacement_text) =
                self.resolve_static_string_value(replacement_expression)
            {
                Some(replacement_text)
            } else if let Some(LocalFunctionBinding::User(function_name)) =
                self.resolve_function_binding_from_expression(replacement_expression)
            {
                let Some(user_function) = self.user_function(&function_name).cloned() else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(true);
                };
                let Some(match_index) = source_text.find(&search_text) else {
                    self.emit_static_string_literal(&source_text)?;
                    return Ok(true);
                };
                let callback_argument_expressions = vec![
                    Expression::String(search_text.clone()),
                    Expression::Number(match_index as f64),
                    Expression::String(source_text.clone()),
                ];
                let callback_arguments = callback_argument_expressions
                    .iter()
                    .cloned()
                    .map(CallArgument::Expression)
                    .collect::<Vec<_>>();
                self.emit_user_function_call(&user_function, &callback_arguments)?;
                self.state.emission.output.instructions.push(0x1a);
                let this_binding = if user_function.strict {
                    Expression::Undefined
                } else {
                    Expression::This
                };
                self.resolve_function_binding_static_return_expression_with_call_frame(
                    &LocalFunctionBinding::User(function_name),
                    &callback_argument_expressions,
                    &this_binding,
                )
                .and_then(|value| self.resolve_static_string_value(&value))
            } else {
                None
            };

            if let Some(replacement_text) = replacement_text {
                self.emit_static_string_literal(&source_text.replacen(
                    &search_text,
                    &replacement_text,
                    1,
                ))?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}
