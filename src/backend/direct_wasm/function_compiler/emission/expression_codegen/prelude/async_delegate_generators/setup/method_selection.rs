use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn async_yield_delegate_uses_async_iterator_method(
        &self,
        plan: &AsyncYieldDelegateGeneratorPlan,
        async_iterator_property: &Expression,
    ) -> bool {
        if let Some(getter_binding) =
            self.resolve_member_getter_binding(&plan.delegate_expression, async_iterator_property)
        {
            return self
                .resolve_function_binding_static_return_expression_with_call_frame(
                    &getter_binding,
                    &[],
                    &plan.delegate_expression,
                )
                .map(|value| !matches!(value, Expression::Null | Expression::Undefined))
                .unwrap_or(false);
        }
        self.resolve_member_function_binding(&plan.delegate_expression, async_iterator_property)
            .is_some()
    }

    pub(in crate::backend::direct_wasm) fn emit_async_yield_delegate_setup(
        &mut self,
        plan: &AsyncYieldDelegateGeneratorPlan,
        uses_async_iterator_method: bool,
        async_iterator_member: &Expression,
        iterator_member: &Expression,
        delegate_iterator_method_name: &str,
        delegate_iterator_name: &str,
        async_iterator_property: &Expression,
    ) -> DirectResult<()> {
        self.with_current_user_function_name(Some(plan.function_name.clone()), |compiler| {
            for effect in &plan.prefix_effects {
                compiler.emit_statement(effect)?;
            }
            if compiler
                .resolve_member_getter_binding(&plan.delegate_expression, async_iterator_property)
                .is_some()
                && !uses_async_iterator_method
            {
                compiler.emit_statement(&Statement::Expression(async_iterator_member.clone()))?;
            }
            let delegate_iterator_member = if uses_async_iterator_method {
                async_iterator_member.clone()
            } else {
                iterator_member.clone()
            };
            compiler.with_restored_function_static_binding_metadata(|compiler| {
                compiler.emit_statement(&Statement::Assign {
                    name: delegate_iterator_method_name.to_string(),
                    value: delegate_iterator_member.clone(),
                })
            })?;
            let delegate_iterator_call = Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new(Expression::Identifier(
                        delegate_iterator_method_name.to_string(),
                    )),
                    property: Box::new(Expression::String("call".to_string())),
                }),
                arguments: vec![CallArgument::Expression(plan.delegate_expression.clone())],
            };
            compiler.with_restored_function_static_binding_metadata(|compiler| {
                compiler.emit_statement(&Statement::Assign {
                    name: delegate_iterator_name.to_string(),
                    value: delegate_iterator_call.clone(),
                })
            })?;
            Ok(())
        })
    }
}
