use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_iterator_step_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<IteratorStepBinding> {
        if let Expression::Identifier(name) = expression {
            if let Some(binding) = self
                .state
                .speculation
                .static_semantics
                .local_iterator_step_binding(name)
            {
                return Some(binding.clone());
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && let Some(binding) = self
                    .state
                    .speculation
                    .static_semantics
                    .local_iterator_step_binding(&resolved_name)
            {
                return Some(binding.clone());
            }
        }
        let Expression::Identifier(name) = self.resolve_bound_alias_expression(expression)? else {
            return None;
        };
        self.state
            .speculation
            .static_semantics
            .local_iterator_step_binding(&name)
            .cloned()
    }
}
