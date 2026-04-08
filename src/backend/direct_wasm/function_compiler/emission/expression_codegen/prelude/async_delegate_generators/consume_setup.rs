use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn prepare_async_yield_delegate_generator_consumption(
        &mut self,
        object: &Expression,
        property_name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<AsyncDelegateConsumptionPreparation> {
        let Expression::Identifier(name) = object else {
            return Ok(AsyncDelegateConsumptionPreparation::NotApplicable);
        };
        let Some(binding_name) = self.resolve_local_array_iterator_binding_name(name) else {
            return Ok(AsyncDelegateConsumptionPreparation::NotApplicable);
        };
        let Some(binding) = self
            .state
            .speculation
            .static_semantics
            .local_array_iterator_binding(&binding_name)
            .cloned()
        else {
            return Ok(AsyncDelegateConsumptionPreparation::NotApplicable);
        };
        let current_static_index = binding.static_index;
        let IteratorSourceKind::AsyncYieldDelegateGenerator {
            plan,
            delegate_iterator_name,
            delegate_next_name,
            delegate_completion_name,
            uses_async_iterator_method: stored_uses_async_iterator_method,
            snapshot_bindings,
        } = &binding.source
        else {
            return Ok(AsyncDelegateConsumptionPreparation::NotApplicable);
        };
        if property_name != "next" && property_name != "return" && property_name != "throw" {
            return Ok(AsyncDelegateConsumptionPreparation::NotApplicable);
        }

        let delegate_iterator_expression = Expression::Identifier(delegate_iterator_name.clone());
        let delegate_completion_expression =
            Expression::Identifier(delegate_completion_name.clone());
        let mut delegate_snapshot_bindings = snapshot_bindings.clone();
        if let Some(snapshot_bindings) = delegate_snapshot_bindings.as_mut() {
            self.refresh_async_delegate_snapshot_bindings_from_visible_state(snapshot_bindings);
            self.sync_async_delegate_snapshot_bindings(snapshot_bindings)?;
        }
        let scoped_snapshot_names =
            if let Some(snapshot_bindings) = delegate_snapshot_bindings.as_ref() {
                self.push_async_delegate_snapshot_scope_bindings(snapshot_bindings)?
            } else {
                Vec::new()
            };
        let snapshot_current_argument =
            if property_name == "next" && delegate_snapshot_bindings.is_none() {
                Expression::Undefined
            } else {
                self.materialize_static_expression(&self.promise_argument_expression(arguments, 0))
            };
        let step_result_name =
            self.allocate_named_hidden_local("async_delegate_result", StaticValueKind::Object);
        let promise_value_name =
            self.allocate_named_hidden_local("async_delegate_value", StaticValueKind::Unknown);
        let promise_done_name =
            self.allocate_named_hidden_local("async_delegate_done", StaticValueKind::Bool);

        if property_name == "next"
            && matches!(current_static_index, Some(index) if index >= 2)
            && delegate_snapshot_bindings
                .as_ref()
                .is_some_and(|bindings| !bindings.contains_key(delegate_iterator_name.as_str()))
        {
            let returned_done_expression = delegate_snapshot_bindings
                .as_ref()
                .and_then(|bindings| bindings.get(&promise_done_name).cloned())
                .unwrap_or(Expression::Bool(true));
            let returned_value_expression = delegate_snapshot_bindings
                .as_ref()
                .and_then(|bindings| bindings.get(&promise_value_name).cloned())
                .unwrap_or(Expression::Undefined);
            if let Some(snapshot_bindings) = delegate_snapshot_bindings.as_ref() {
                self.sync_async_delegate_snapshot_bindings(snapshot_bindings)?;
            }
            self.pop_async_delegate_snapshot_scope_bindings(&scoped_snapshot_names);
            return Ok(AsyncDelegateConsumptionPreparation::Outcome(
                StaticEvalOutcome::Value(Expression::Object(vec![
                    ObjectEntry::Data {
                        key: Expression::String("done".to_string()),
                        value: returned_done_expression,
                    },
                    ObjectEntry::Data {
                        key: Expression::String("value".to_string()),
                        value: returned_value_expression,
                    },
                ])),
            ));
        }

        let current_arg_name =
            self.allocate_named_hidden_local("async_delegate_arg", StaticValueKind::Unknown);
        let delegate_iterator_method_name = self.allocate_named_hidden_local(
            "async_delegate_iterator_method",
            StaticValueKind::Unknown,
        );
        let current_argument_expression = self.promise_argument_expression(arguments, 0);

        if property_name == "next" {
            match current_static_index {
                Some(0) => {
                    self.emit_statement(&Statement::Assign {
                        name: current_arg_name.clone(),
                        value: Expression::Undefined,
                    })?;
                }
                Some(_) => {
                    self.emit_statement(&Statement::Assign {
                        name: current_arg_name.clone(),
                        value: current_argument_expression.clone(),
                    })?;
                }
                None => {
                    self.push_local_get(binding.index_local);
                    self.push_i32_const(0);
                    self.push_binary_op(BinaryOp::Equal)?;
                    self.state.emission.output.instructions.push(0x04);
                    self.state
                        .emission
                        .output
                        .instructions
                        .push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.emit_statement(&Statement::Assign {
                        name: current_arg_name.clone(),
                        value: Expression::Undefined,
                    })?;
                    self.state.emission.output.instructions.push(0x05);
                    self.with_restored_function_static_binding_metadata(|compiler| {
                        compiler.emit_statement(&Statement::Assign {
                            name: current_arg_name.clone(),
                            value: current_argument_expression.clone(),
                        })
                    })?;
                    self.state.emission.output.instructions.push(0x0b);
                    self.pop_control_frame();
                }
            }
        } else {
            self.emit_statement(&Statement::Assign {
                name: current_arg_name.clone(),
                value: current_argument_expression.clone(),
            })?;
        }

        let async_iterator_property = self.materialize_static_expression(&Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("asyncIterator".to_string())),
        });
        let iterator_property = self.materialize_static_expression(&symbol_iterator_expression());
        let async_iterator_member = Expression::Member {
            object: Box::new(plan.delegate_expression.clone()),
            property: Box::new(async_iterator_property.clone()),
        };
        let iterator_member = Expression::Member {
            object: Box::new(plan.delegate_expression.clone()),
            property: Box::new(iterator_property.clone()),
        };
        let uses_async_iterator_method = stored_uses_async_iterator_method.unwrap_or_else(|| {
            self.async_yield_delegate_uses_async_iterator_method(plan, &async_iterator_property)
        });
        if delegate_snapshot_bindings.is_none() {
            match self.initialize_async_yield_delegate_snapshot_bindings(
                plan,
                &async_iterator_property,
                &iterator_property,
                &delegate_iterator_method_name,
                delegate_iterator_name,
            )? {
                Some(InitialDelegateSnapshotBindings::Ready { bindings }) => {
                    delegate_snapshot_bindings = Some(bindings);
                }
                Some(InitialDelegateSnapshotBindings::Throw {
                    throw_value,
                    bindings,
                }) => {
                    self.persist_async_yield_delegate_generator_snapshot_state(
                        &binding_name,
                        Some(2),
                        Some(bindings.clone()),
                    );
                    self.sync_async_delegate_snapshot_bindings(&bindings)?;
                    self.pop_async_delegate_snapshot_scope_bindings(&scoped_snapshot_names);
                    return Ok(AsyncDelegateConsumptionPreparation::Outcome(
                        StaticEvalOutcome::Throw(throw_value),
                    ));
                }
                None => self.emit_async_yield_delegate_setup(
                    plan,
                    uses_async_iterator_method,
                    &async_iterator_member,
                    &iterator_member,
                    &delegate_iterator_method_name,
                    delegate_iterator_name,
                    &async_iterator_property,
                )?,
            }
        } else if let Some(snapshot_bindings) = delegate_snapshot_bindings.as_ref() {
            self.sync_async_delegate_snapshot_bindings(snapshot_bindings)?;
        }

        Ok(AsyncDelegateConsumptionPreparation::Ready(
            PreparedAsyncDelegateConsumption {
                binding_name,
                current_static_index,
                index_local: binding.index_local,
                property_name: property_name.to_string(),
                plan: plan.clone(),
                delegate_iterator_name: delegate_iterator_name.clone(),
                delegate_next_name: delegate_next_name.clone(),
                delegate_completion_name: delegate_completion_name.clone(),
                delegate_iterator_expression,
                delegate_completion_expression,
                delegate_snapshot_bindings,
                scoped_snapshot_names,
                snapshot_current_argument,
                step_result_name,
                promise_value_name,
                promise_done_name,
            },
        ))
    }
}
