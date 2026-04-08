use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_local_array_iterator_source(
        &mut self,
        value: &Expression,
    ) -> Option<IteratorSourceKind> {
        let current_function_is_async_generator = self
            .async_generator_domain()
            .current_function_is_async_generator();
        let source_expression = match value {
            Expression::GetIterator(iterated) => iterated.as_ref(),
            Expression::Call { .. }
                if self
                    .resolve_async_yield_delegate_generator_plan(
                        value,
                        "__ayy_async_delegate_completion",
                    )
                    .is_some() =>
            {
                value
            }
            Expression::Call { .. } if self.resolve_simple_generator_source(value).is_some() => {
                value
            }
            _ => return None,
        };
        if let Some(source) = self.resolve_iterator_source_kind(source_expression) {
            return Some(source);
        }
        if let Expression::GetIterator(iterated) = value
            && let Some((steps, completion_effects)) = self
                .resolve_simple_yield_delegate_source(iterated, current_function_is_async_generator)
            && completion_effects.is_empty()
            && steps
                .iter()
                .all(|step| matches!(step.outcome, SimpleGeneratorStepOutcome::Throw(_)))
        {
            return Some(IteratorSourceKind::SimpleGenerator {
                is_async: current_function_is_async_generator,
                steps,
                completion_effects,
                completion_value: Expression::Undefined,
            });
        }
        let delegate_completion_name =
            self.allocate_named_hidden_local("async_delegate_completion", StaticValueKind::Unknown);
        let plan = self.resolve_async_yield_delegate_generator_plan(
            source_expression,
            &delegate_completion_name,
        )?;
        let mut scope_binding_replacements = HashMap::new();
        for scope_binding in &plan.scope_bindings {
            let hidden_name = self.allocate_named_hidden_local(
                &format!("async_delegate_scope_{scope_binding}"),
                StaticValueKind::Unknown,
            );
            scope_binding_replacements
                .insert(scope_binding.clone(), Expression::Identifier(hidden_name));
        }
        let plan = self.substitute_async_yield_delegate_generator_plan_scope_bindings(
            &plan,
            &scope_binding_replacements,
        );
        let delegate_iterator_name =
            self.allocate_named_hidden_local("async_delegate_iterator", StaticValueKind::Object);
        let delegate_next_name =
            self.allocate_named_hidden_local("async_delegate_next", StaticValueKind::Unknown);
        Some(IteratorSourceKind::AsyncYieldDelegateGenerator {
            plan,
            delegate_iterator_name,
            delegate_next_name,
            delegate_completion_name,
            uses_async_iterator_method: None,
            snapshot_bindings: None,
        })
    }
}
