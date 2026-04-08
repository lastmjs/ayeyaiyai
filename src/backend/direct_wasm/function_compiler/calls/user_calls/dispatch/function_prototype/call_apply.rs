use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_function_prototype_call_or_apply(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Expression::String(property_name) = property else {
            return Ok(false);
        };
        if property_name != "call" && property_name != "apply" {
            return Ok(false);
        }
        if property_name == "call" && self.emit_has_own_property_call(object, arguments)? {
            return Ok(true);
        }

        let Some(function_binding) = self.resolve_function_binding_from_expression(object) else {
            return Ok(false);
        };
        let LocalFunctionBinding::User(function_name) = function_binding else {
            return Ok(false);
        };
        let Some(user_function) = self.user_function(&function_name).cloned() else {
            return Ok(false);
        };
        let capture_slots = self.resolve_function_expression_capture_slots(object);

        let expanded_arguments = self.expand_call_arguments(arguments);
        let raw_this_expression = expanded_arguments
            .first()
            .cloned()
            .unwrap_or(Expression::Undefined);
        let (call_arguments, apply_expression) = if property_name == "call" {
            (
                expanded_arguments
                    .iter()
                    .skip(1)
                    .cloned()
                    .map(CallArgument::Expression)
                    .collect::<Vec<_>>(),
                None,
            )
        } else {
            let apply_expression = expanded_arguments
                .get(1)
                .cloned()
                .unwrap_or(Expression::Undefined);
            let Some(call_arguments) =
                self.expand_apply_call_arguments_from_expression(&apply_expression)
            else {
                return Ok(false);
            };
            (call_arguments, Some(apply_expression))
        };
        let materialized_this_expression = self.materialize_static_expression(&raw_this_expression);
        let materialized_call_arguments = call_arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.materialize_static_expression(expression)
                }
            })
            .collect::<Vec<_>>();
        let call_argument_expressions = call_arguments
            .iter()
            .map(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    expression.clone()
                }
            })
            .collect::<Vec<_>>();

        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);

        if capture_slots.is_none()
            && (user_function.strict || user_function.lexical_this)
            && self.can_inline_user_function_call_with_explicit_call_frame(
                &user_function,
                &materialized_call_arguments,
                &materialized_this_expression,
            )
        {
            if let Some(apply_expression) = &apply_expression {
                self.emit_numeric_expression(apply_expression)?;
                self.state.emission.output.instructions.push(0x1a);
                for extra_argument in expanded_arguments.iter().skip(2) {
                    self.emit_numeric_expression(extra_argument)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
            let result_local = self.allocate_temp_local();
            if self.emit_inline_user_function_summary_with_explicit_call_frame(
                &user_function,
                &call_argument_expressions,
                &materialized_this_expression,
                result_local,
            )? {
                self.push_local_get(result_local);
                return Ok(true);
            }
        }

        let this_hidden_name = if apply_expression.is_some() {
            let this_hidden_name = self.allocate_named_hidden_local(
                "call_apply_this",
                self.infer_value_kind(&raw_this_expression)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let this_hidden_local = self
                .state
                .runtime
                .locals
                .get(&this_hidden_name)
                .copied()
                .expect("fresh call/apply hidden local must exist");
            self.emit_numeric_expression(&raw_this_expression)?;
            self.push_local_set(this_hidden_local);
            Some(this_hidden_name)
        } else {
            None
        };
        if let Some(apply_expression) = &apply_expression {
            self.emit_numeric_expression(apply_expression)?;
            self.state.emission.output.instructions.push(0x1a);
            for extra_argument in expanded_arguments.iter().skip(2) {
                self.emit_numeric_expression(extra_argument)?;
                self.state.emission.output.instructions.push(0x1a);
            }
        }
        let this_expression = this_hidden_name
            .map(Expression::Identifier)
            .unwrap_or_else(|| raw_this_expression.clone());
        self.emit_user_function_call_with_function_this_binding(
            &user_function,
            &call_arguments,
            &this_expression,
            capture_slots.as_ref(),
        )?;
        Ok(true)
    }
}
