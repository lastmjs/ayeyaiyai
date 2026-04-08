use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn close_local_iterator_binding(&mut self, name: &str) {
        let Some(mut binding) = self
            .state
            .speculation
            .static_semantics
            .local_array_iterator_binding(name)
            .cloned()
        else {
            return;
        };
        let (closed_state, closed_static_index) = match &binding.source {
            IteratorSourceKind::StaticArray {
                values,
                length_local,
                runtime_name,
                ..
            } => {
                let closed_static_index = if length_local.is_none() && runtime_name.is_none() {
                    Some(values.len().saturating_add(1))
                } else {
                    None
                };
                (i32::MAX, closed_static_index)
            }
            IteratorSourceKind::SimpleGenerator { steps, .. } => {
                let closed_index = steps.len().saturating_add(1);
                (closed_index as i32, Some(closed_index))
            }
            IteratorSourceKind::AsyncYieldDelegateGenerator { .. } => (2, None),
            IteratorSourceKind::TypedArrayView { .. }
            | IteratorSourceKind::DirectArguments { .. } => (i32::MAX, None),
        };
        self.push_i32_const(closed_state);
        self.push_local_set(binding.index_local);
        binding.static_index = closed_static_index;
        self.state
            .speculation
            .static_semantics
            .set_local_array_iterator_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_argument_iterator_bindings_for_user_call(
        &mut self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) {
        let consumed_indices =
            self.user_function_parameter_iterator_consumption_indices(user_function);
        if consumed_indices.is_empty() {
            return;
        }
        for (index, argument) in arguments.iter().enumerate() {
            if !consumed_indices.contains(&index) {
                continue;
            }
            let Some(name) = (match argument {
                Expression::Identifier(name) => Some(name.clone()),
                _ => match self.resolve_bound_alias_expression(argument) {
                    Some(Expression::Identifier(name)) => Some(name),
                    _ => None,
                },
            }) else {
                continue;
            };
            let Some(binding_name) = self.resolve_local_array_iterator_binding_name(&name) else {
                continue;
            };
            self.close_local_iterator_binding(&binding_name);
        }
    }
}
