use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_local_iterator_step_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Call { callee, arguments } = value else {
            self.state
                .speculation
                .static_semantics
                .clear_local_iterator_step_binding(name);
            return;
        };
        if !arguments.is_empty() {
            self.state
                .speculation
                .static_semantics
                .clear_local_iterator_step_binding(name);
            return;
        }
        let Expression::Member { object, property } = callee.as_ref() else {
            self.state
                .speculation
                .static_semantics
                .clear_local_iterator_step_binding(name);
            return;
        };
        if !matches!(property.as_ref(), Expression::String(property_name) if property_name == "next")
        {
            self.state
                .speculation
                .static_semantics
                .clear_local_iterator_step_binding(name);
            return;
        }
        let Expression::Identifier(iterator_name) = object.as_ref() else {
            self.state
                .speculation
                .static_semantics
                .clear_local_iterator_step_binding(name);
            return;
        };
        let iterator_binding_name = self
            .resolve_local_array_iterator_binding_name(iterator_name)
            .unwrap_or_else(|| iterator_name.clone());
        let Some(mut iterator_binding) = self
            .state
            .speculation
            .static_semantics
            .local_array_iterator_binding(&iterator_binding_name)
            .cloned()
        else {
            self.state
                .speculation
                .static_semantics
                .clear_local_iterator_step_binding(name);
            return;
        };
        let uses_previous_static_index = self
            .state
            .speculation
            .static_semantics
            .last_bound_user_function_call
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.function_name == "__ayy_simple_generator_next"
                    && snapshot
                        .source_expression
                        .as_ref()
                        .is_some_and(|source| static_expression_matches(source, value))
            });
        let (done_local, value_local) = match self
            .state
            .speculation
            .static_semantics
            .local_iterator_step_binding(name)
        {
            Some(IteratorStepBinding::Runtime {
                done_local,
                value_local,
                ..
            }) => (*done_local, *value_local),
            _ => (self.allocate_temp_local(), self.allocate_temp_local()),
        };
        let function_binding = self.resolve_iterator_step_function_binding(&iterator_binding);
        let current_static_index = if uses_previous_static_index {
            iterator_binding
                .static_index
                .map(|index| index.saturating_sub(1))
        } else {
            iterator_binding.static_index
        };
        let sent_value = Expression::Undefined;

        if uses_previous_static_index
            && let IteratorSourceKind::SimpleGenerator { .. } = &iterator_binding.source
            && let Some(index) = current_static_index
        {
            let (static_done, static_value) = self.emit_previous_simple_generator_iterator_step(
                &mut iterator_binding,
                index,
                &sent_value,
                done_local,
                value_local,
            );
            self.state
                .speculation
                .static_semantics
                .set_local_array_iterator_binding(&iterator_binding_name, iterator_binding);
            self.state
                .speculation
                .static_semantics
                .set_local_iterator_step_binding(
                    name,
                    IteratorStepBinding::Runtime {
                        done_local,
                        value_local,
                        function_binding,
                        static_done,
                        static_value,
                    },
                );
            self.state
                .speculation
                .static_semantics
                .set_local_kind(name, StaticValueKind::Object);
            return;
        }

        let (static_done, static_value) = self.resolve_iterator_step_static_outcome(
            &iterator_binding,
            current_static_index,
            &sent_value,
        );

        let current_index_local = self.allocate_temp_local();
        self.push_local_get(iterator_binding.index_local);
        self.push_local_set(current_index_local);

        self.emit_runtime_iterator_step_source_update(
            &mut iterator_binding,
            current_static_index,
            current_index_local,
            &sent_value,
            done_local,
            value_local,
        );

        self.state
            .speculation
            .static_semantics
            .set_local_array_iterator_binding(&iterator_binding_name, iterator_binding);
        self.state
            .speculation
            .static_semantics
            .set_local_iterator_step_binding(
                name,
                IteratorStepBinding::Runtime {
                    done_local,
                    value_local,
                    function_binding,
                    static_done,
                    static_value,
                },
            );
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
    }

    fn resolve_iterator_step_function_binding(
        &self,
        iterator_binding: &ArrayIteratorBinding,
    ) -> Option<LocalFunctionBinding> {
        match &iterator_binding.source {
            IteratorSourceKind::StaticArray {
                values, keys_only, ..
            } if !keys_only => {
                let bindings = values
                    .iter()
                    .flatten()
                    .map(|value| self.resolve_function_binding_from_expression(value))
                    .collect::<Option<Vec<_>>>();
                bindings.and_then(|bindings| {
                    if bindings.is_empty() {
                        None
                    } else if bindings
                        .iter()
                        .all(|binding| binding == bindings.first().expect("not empty"))
                    {
                        bindings.first().cloned()
                    } else if are_function_constructor_bindings(&bindings) {
                        Some(LocalFunctionBinding::Builtin(
                            FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN.to_string(),
                        ))
                    } else {
                        None
                    }
                })
            }
            _ => None,
        }
    }
}
