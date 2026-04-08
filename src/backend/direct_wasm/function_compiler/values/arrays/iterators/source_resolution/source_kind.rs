use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn is_async_generator_call_expression(
        &self,
        expression: &Expression,
    ) -> bool {
        let Expression::Call { callee, .. } = expression else {
            return false;
        };
        let Some(LocalFunctionBinding::User(function_name)) =
            self.resolve_function_binding_from_expression(callee)
        else {
            return false;
        };
        self.user_function(&function_name)
            .is_some_and(|function| matches!(function.kind, FunctionKind::AsyncGenerator))
    }

    pub(in crate::backend::direct_wasm) fn tracked_direct_arguments_prefix_len(&self) -> u32 {
        let mut indices = self
            .state
            .parameters
            .arguments_slots
            .keys()
            .copied()
            .collect::<Vec<_>>();
        indices.sort_unstable();
        let mut next_index = 0;
        for index in indices {
            if index != next_index {
                break;
            }
            next_index += 1;
        }
        next_index
    }

    pub(in crate::backend::direct_wasm) fn resolve_iterator_source_kind(
        &self,
        expression: &Expression,
    ) -> Option<IteratorSourceKind> {
        let structural_key = format!("{expression:?}");
        let inserted = ACTIVE_ITERATOR_SOURCE_SHAPES
            .with(|active| active.borrow_mut().insert(structural_key.clone()));
        if !inserted {
            return None;
        }
        let _guard = IteratorSourceGuard {
            key: structural_key,
        };
        if self.is_direct_arguments_object(expression) {
            return Some(IteratorSourceKind::DirectArguments {
                tracked_prefix_len: self.tracked_direct_arguments_prefix_len(),
            });
        }
        if let Expression::Identifier(name) = expression
            && self
                .state
                .speculation
                .static_semantics
                .has_local_typed_array_view_binding(name)
        {
            return Some(IteratorSourceKind::TypedArrayView { name: name.clone() });
        }
        if let Some((steps, completion_effects, completion_value)) =
            self.resolve_array_prototype_simple_generator_source(expression)
        {
            return Some(IteratorSourceKind::SimpleGenerator {
                is_async: false,
                steps,
                completion_effects,
                completion_value,
            });
        }
        if let Some(source) = self.resolve_static_array_iterator_source_kind(expression, false) {
            return Some(source);
        }
        if let Expression::Identifier(name) = expression
            && let Some(value) = self
                .state
                .speculation
                .static_semantics
                .local_value_binding(name)
                .or_else(|| self.global_value_binding(name))
            && !static_expression_matches(value, expression)
            && let Some(source) = self.resolve_iterator_source_kind(value)
        {
            return Some(source);
        }
        if let Some((steps, completion_effects, completion_value)) =
            self.resolve_simple_generator_source(expression)
        {
            let is_async = matches!(
                expression,
                Expression::Call { callee, .. }
                    if self
                        .resolve_function_binding_from_expression(callee)
                        .and_then(|binding| match binding {
                            LocalFunctionBinding::User(function_name) => {
                                self.user_function(&function_name)
                            }
                            LocalFunctionBinding::Builtin(_) => None,
                        })
                        .is_some_and(|function| matches!(function.kind, FunctionKind::AsyncGenerator))
            );
            return Some(IteratorSourceKind::SimpleGenerator {
                is_async,
                steps,
                completion_effects,
                completion_value,
            });
        }
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.resolve_iterator_source_kind(&materialized);
        }
        if let Some(source) = self.resolve_iterator_source_call_shape_kind(expression) {
            return Some(source);
        }
        if let Some((_, returned_expression, _)) =
            self.analyze_effectful_iterator_source_call(expression)
        {
            return self.resolve_iterator_source_kind(&returned_expression);
        }
        let binding = self.resolve_static_iterable_binding_from_expression(expression)?;
        Some(IteratorSourceKind::StaticArray {
            values: binding.values,
            keys_only: false,
            length_local: None,
            runtime_name: None,
        })
    }

    fn resolve_static_array_iterator_source_kind(
        &self,
        expression: &Expression,
        keys_only: bool,
    ) -> Option<IteratorSourceKind> {
        let array_binding = self.resolve_array_binding_from_expression(expression)?;
        let length_local = match expression {
            Expression::Identifier(name)
                if self.is_named_global_array_binding(name)
                    && (!self.state.speculation.execution_context.top_level_function
                        || self.uses_global_runtime_array_state(name)) =>
            {
                None
            }
            _ => self.runtime_array_length_local_for_expression(expression),
        };
        Some(IteratorSourceKind::StaticArray {
            values: array_binding.values,
            keys_only,
            length_local,
            runtime_name: match expression {
                Expression::Identifier(name)
                    if self
                        .runtime_array_length_local_for_expression(expression)
                        .is_some()
                        || (self.is_named_global_array_binding(name)
                            && (!self.state.speculation.execution_context.top_level_function
                                || self.uses_global_runtime_array_state(name))) =>
                {
                    Some(name.clone())
                }
                _ => None,
            },
        })
    }

    fn resolve_iterator_source_call_shape_kind(
        &self,
        expression: &Expression,
    ) -> Option<IteratorSourceKind> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        let Expression::Member { object, property } = callee.as_ref() else {
            return None;
        };
        if is_symbol_iterator_expression(property) {
            return self.resolve_iterator_source_kind(object);
        }
        if matches!(property.as_ref(), Expression::String(name) if name == "keys") {
            return self.resolve_static_array_iterator_source_kind(object, true);
        }
        None
    }
}
