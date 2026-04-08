use super::*;

impl<'a> FunctionCompiler<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::backend::direct_wasm) fn initialize_async_yield_delegate_snapshot_bindings(
        &mut self,
        plan: &AsyncYieldDelegateGeneratorPlan,
        async_iterator_property: &Expression,
        iterator_property: &Expression,
        delegate_iterator_method_name: &str,
        delegate_iterator_name: &str,
    ) -> DirectResult<Option<InitialDelegateSnapshotBindings>> {
        let delegate_function_name = Some(plan.function_name.as_str());
        self.with_restored_function_static_binding_metadata(|compiler| {
            let mut initial_snapshot_bindings = HashMap::new();
            for name in collect_referenced_binding_names_from_statements(&plan.prefix_effects) {
                if !compiler.should_sync_async_delegate_snapshot_binding(&name) {
                    continue;
                }
                initial_snapshot_bindings
                    .entry(name.clone())
                    .or_insert_with(|| {
                        compiler.materialize_static_expression(&Expression::Identifier(name))
                    });
            }
            compiler.execute_bound_snapshot_statements(
                &plan.prefix_effects,
                &mut initial_snapshot_bindings,
                Some(&plan.function_name),
            );
            let async_iterator_getter_binding = compiler
                .resolve_member_getter_binding(&plan.delegate_expression, async_iterator_property);
            let iterator_getter_binding = compiler
                .resolve_member_getter_binding(&plan.delegate_expression, iterator_property);
            let async_iterator_snapshot =
                if let Some(getter_binding) = async_iterator_getter_binding.as_ref() {
                    match compiler.resolve_bound_snapshot_function_outcome_with_arguments_and_this(
                        getter_binding,
                        &initial_snapshot_bindings,
                        &[],
                        &plan.delegate_expression,
                    ) {
                        Some((StaticEvalOutcome::Value(iterator_method), updated_bindings)) => {
                            Some((iterator_method, updated_bindings))
                        }
                        Some((StaticEvalOutcome::Throw(throw_value), updated_bindings)) => {
                            return Ok(Some(InitialDelegateSnapshotBindings::Throw {
                                throw_value,
                                bindings: updated_bindings,
                            }));
                        }
                        None => {
                            if let Some(StaticEvalOutcome::Throw(throw_value)) = compiler
                                .resolve_static_function_outcome_from_binding_with_context(
                                    getter_binding,
                                    &[],
                                    delegate_function_name,
                                )
                            {
                                return Ok(Some(InitialDelegateSnapshotBindings::Throw {
                                    throw_value,
                                    bindings: initial_snapshot_bindings,
                                }));
                            }
                            None
                        }
                    }
                } else {
                    None
                };
            let iterator_binding_snapshot = if let Some((iterator_method, updated_bindings)) =
                async_iterator_snapshot
            {
                initial_snapshot_bindings = updated_bindings;
                if matches!(iterator_method, Expression::Null | Expression::Undefined) {
                    if let Some(getter_binding) = compiler
                        .resolve_member_getter_binding(&plan.delegate_expression, iterator_property)
                    {
                        match compiler
                            .resolve_bound_snapshot_function_outcome_with_arguments_and_this(
                                &getter_binding,
                                &initial_snapshot_bindings,
                                &[],
                                &plan.delegate_expression,
                            ) {
                            Some((StaticEvalOutcome::Value(iterator_method), updated_bindings)) => {
                                compiler
                                    .resolve_function_binding_from_expression(&iterator_method)
                                    .map(|function_binding| (function_binding, updated_bindings))
                            }
                            Some((StaticEvalOutcome::Throw(throw_value), updated_bindings)) => {
                                return Ok(Some(InitialDelegateSnapshotBindings::Throw {
                                    throw_value,
                                    bindings: updated_bindings,
                                }));
                            }
                            None => match compiler
                                .resolve_static_function_outcome_from_binding_with_context(
                                    &getter_binding,
                                    &[],
                                    Some(&plan.function_name),
                                ) {
                                Some(StaticEvalOutcome::Throw(throw_value)) => {
                                    return Ok(Some(InitialDelegateSnapshotBindings::Throw {
                                        throw_value,
                                        bindings: initial_snapshot_bindings,
                                    }));
                                }
                                _ => None,
                            },
                        }
                    } else {
                        compiler
                            .resolve_member_function_binding(
                                &plan.delegate_expression,
                                iterator_property,
                            )
                            .map(|function_binding| (function_binding, initial_snapshot_bindings))
                    }
                } else {
                    compiler
                        .resolve_function_binding_from_expression(&iterator_method)
                        .map(|function_binding| (function_binding, initial_snapshot_bindings))
                }
            } else if async_iterator_getter_binding.is_none()
                && let Some(function_binding) = compiler.resolve_member_function_binding(
                    &plan.delegate_expression,
                    async_iterator_property,
                )
            {
                Some((function_binding, initial_snapshot_bindings))
            } else if let Some(getter_binding) = iterator_getter_binding.as_ref() {
                match compiler.resolve_bound_snapshot_function_outcome_with_arguments_and_this(
                    getter_binding,
                    &initial_snapshot_bindings,
                    &[],
                    &plan.delegate_expression,
                ) {
                    Some((StaticEvalOutcome::Value(iterator_method), updated_bindings)) => compiler
                        .resolve_function_binding_from_expression(&iterator_method)
                        .map(|function_binding| (function_binding, updated_bindings)),
                    Some((StaticEvalOutcome::Throw(throw_value), updated_bindings)) => {
                        return Ok(Some(InitialDelegateSnapshotBindings::Throw {
                            throw_value,
                            bindings: updated_bindings,
                        }));
                    }
                    None => {
                        if let Some(StaticEvalOutcome::Throw(throw_value)) = compiler
                            .resolve_static_function_outcome_from_binding_with_context(
                                getter_binding,
                                &[],
                                delegate_function_name,
                            )
                        {
                            return Ok(Some(InitialDelegateSnapshotBindings::Throw {
                                throw_value,
                                bindings: initial_snapshot_bindings,
                            }));
                        }
                        None
                    }
                }
            } else {
                compiler
                    .resolve_member_function_binding(&plan.delegate_expression, iterator_property)
                    .map(|function_binding| (function_binding, initial_snapshot_bindings))
            };
            if let Some((function_binding, iterator_snapshot_bindings)) = iterator_binding_snapshot
            {
                if let LocalFunctionBinding::User(function_name) = &function_binding {
                    let function_expression = Expression::Identifier(function_name.clone());
                    compiler.update_local_value_binding(
                        delegate_iterator_method_name,
                        &function_expression,
                    );
                    compiler.update_local_function_binding(
                        delegate_iterator_method_name,
                        &function_expression,
                    );
                }
                match compiler.resolve_bound_snapshot_function_outcome_with_arguments_and_this(
                    &function_binding,
                    &iterator_snapshot_bindings,
                    &[],
                    &plan.delegate_expression,
                ) {
                    Some((
                        StaticEvalOutcome::Value(static_delegate_iterator),
                        mut updated_bindings,
                    )) => {
                        updated_bindings.insert(
                            delegate_iterator_name.to_string(),
                            static_delegate_iterator.clone(),
                        );
                        if let LocalFunctionBinding::User(function_name) = &function_binding {
                            compiler
                                .state
                                .speculation
                                .static_semantics
                                .last_bound_user_function_call =
                                Some(BoundUserFunctionCallSnapshot {
                                    function_name: function_name.clone(),
                                    source_expression: Some(Expression::Call {
                                        callee: Box::new(Expression::Identifier(
                                            delegate_iterator_method_name.to_string(),
                                        )),
                                        arguments: Vec::new(),
                                    }),
                                    result_expression: Some(static_delegate_iterator.clone()),
                                    updated_bindings: updated_bindings.clone(),
                                });
                        }
                        compiler.update_local_value_binding(
                            delegate_iterator_name,
                            &static_delegate_iterator,
                        );
                        compiler.update_local_object_binding(
                            delegate_iterator_name,
                            &static_delegate_iterator,
                        );
                        compiler.update_object_literal_member_bindings_for_value(
                            delegate_iterator_name,
                            &static_delegate_iterator,
                        );
                        Ok(Some(InitialDelegateSnapshotBindings::Ready {
                            bindings: updated_bindings,
                        }))
                    }
                    Some((StaticEvalOutcome::Throw(throw_value), updated_bindings)) => {
                        Ok(Some(InitialDelegateSnapshotBindings::Throw {
                            throw_value,
                            bindings: updated_bindings,
                        }))
                    }
                    None => Ok(None),
                }
            } else {
                Ok(None)
            }
        })
    }
}
