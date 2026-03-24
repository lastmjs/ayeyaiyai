use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn materialize_static_expression(
        &self,
        expression: &Expression,
    ) -> Expression {
        let guard_key = expression as *const Expression as usize;
        {
            let mut active = self.materializing_expression_keys.borrow_mut();
            if !active.insert(guard_key) {
                return expression.clone();
            }
        }
        let _guard = MaterializationGuard {
            active: &self.materializing_expression_keys,
            key: guard_key,
        };
        match expression {
            Expression::Identifier(name) => {
                if self.with_scope_blocks_static_identifier_resolution(name) {
                    return Expression::Identifier(name.clone());
                }
                if self.local_object_bindings.contains_key(name)
                    || self.module.global_object_bindings.contains_key(name)
                    || self.local_prototype_object_bindings.contains_key(name)
                    || self
                        .module
                        .global_prototype_object_bindings
                        .contains_key(name)
                {
                    return expression.clone();
                }
                if self.local_array_bindings.contains_key(name)
                    || self.module.global_array_bindings.contains_key(name)
                    || self.local_typed_array_view_bindings.contains_key(name)
                {
                    return expression.clone();
                }
                if let Some(symbol_identity) = self.resolve_symbol_identity_expression(expression) {
                    return symbol_identity;
                }
                if self
                    .local_value_bindings
                    .get(name)
                    .or_else(|| self.module.global_value_bindings.get(name))
                    .is_some_and(|value| {
                        matches!(
                            value,
                            Expression::Call { callee, .. }
                                if matches!(callee.as_ref(), Expression::Identifier(symbol_name)
                                    if symbol_name == "Symbol"
                                        && self.is_unshadowed_builtin_identifier(symbol_name))
                        )
                    })
                {
                    return Expression::Identifier(name.clone());
                }
                if let Some(resolved) = self.resolve_bound_alias_expression(expression) {
                    if !static_expression_matches(&resolved, expression) {
                        let mut referenced_names = HashSet::new();
                        collect_referenced_binding_names_from_expression(
                            &resolved,
                            &mut referenced_names,
                        );
                        if referenced_names.contains(name) {
                            return Expression::Identifier(name.clone());
                        }
                        return self.materialize_static_expression(&resolved);
                    }
                }
                expression.clone()
            }
            Expression::Member { object, property } => {
                if let Some(step_binding) =
                    self.resolve_iterator_step_binding_from_expression(object)
                    && let Expression::String(property_name) = property.as_ref()
                {
                    match (property_name.as_str(), step_binding) {
                        (
                            "done",
                            IteratorStepBinding::Runtime {
                                static_done: Some(done),
                                ..
                            },
                        ) => return Expression::Bool(done),
                        (
                            "value",
                            IteratorStepBinding::Runtime {
                                static_value: Some(value),
                                ..
                            },
                        ) => return self.materialize_static_expression(&value),
                        _ => {}
                    }
                }
                if self.expression_uses_runtime_dynamic_binding(object) {
                    return Expression::Member {
                        object: Box::new(self.materialize_static_expression(object)),
                        property: Box::new(self.materialize_static_expression(property)),
                    };
                }
                if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
                    if matches!(property.as_ref(), Expression::String(text) if text == "length") {
                        if self
                            .runtime_array_length_local_for_expression(object)
                            .is_some()
                        {
                            return Expression::Member {
                                object: Box::new(self.materialize_static_expression(object)),
                                property: Box::new(self.materialize_static_expression(property)),
                            };
                        }
                        return Expression::Number(array_binding.values.len() as f64);
                    }
                    if let Some(index) = argument_index_from_expression(property) {
                        let has_runtime_array_state = self
                            .runtime_array_length_local_for_expression(object)
                            .is_some()
                            || matches!(
                                object.as_ref(),
                                Expression::Identifier(name)
                                    if self.is_named_global_array_binding(name)
                                        && self.uses_global_runtime_array_state(name)
                            );
                        if has_runtime_array_state {
                            return Expression::Member {
                                object: Box::new(self.materialize_static_expression(object)),
                                property: Box::new(self.materialize_static_expression(property)),
                            };
                        }
                        if let Some(Some(value)) = array_binding.values.get(index as usize) {
                            return self.materialize_static_expression(value);
                        }
                        return Expression::Undefined;
                    }
                }
                if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                    let materialized_property = self.materialize_static_expression(property);
                    if let Some(value) =
                        object_binding_lookup_value(&object_binding, &materialized_property)
                    {
                        return self.materialize_static_expression(value);
                    }
                    if static_property_name_from_expression(&materialized_property).is_some()
                        || object_binding_has_property(&object_binding, &materialized_property)
                    {
                        return Expression::Undefined;
                    }
                }
                if let Expression::String(text) = object.as_ref() {
                    if let Some(index) = argument_index_from_expression(property) {
                        return text
                            .chars()
                            .nth(index as usize)
                            .map(|character| Expression::String(character.to_string()))
                            .unwrap_or(Expression::Undefined);
                    }
                }
                Expression::Member {
                    object: Box::new(self.materialize_static_expression(object)),
                    property: Box::new(self.materialize_static_expression(property)),
                }
            }
            Expression::Unary { op, expression } => Expression::Unary {
                op: *op,
                expression: Box::new(self.materialize_static_expression(expression)),
            },
            Expression::Binary { op, left, right } => Expression::Binary {
                op: *op,
                left: Box::new(self.materialize_static_expression(left)),
                right: Box::new(self.materialize_static_expression(right)),
            },
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let materialized_condition = self.materialize_static_expression(condition);
                if let Some(condition_value) =
                    self.resolve_static_if_condition_value(&materialized_condition)
                {
                    let branch = if condition_value {
                        then_expression.as_ref()
                    } else {
                        else_expression.as_ref()
                    };
                    return self.materialize_static_expression(branch);
                }
                Expression::Conditional {
                    condition: Box::new(materialized_condition),
                    then_expression: Box::new(self.materialize_static_expression(then_expression)),
                    else_expression: Box::new(self.materialize_static_expression(else_expression)),
                }
            }
            Expression::Sequence(expressions) => Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| self.materialize_static_expression(expression))
                    .collect(),
            ),
            Expression::Array(elements) => Expression::Array(
                elements
                    .iter()
                    .map(|element| match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            crate::ir::hir::ArrayElement::Expression(
                                self.materialize_static_expression(expression),
                            )
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            crate::ir::hir::ArrayElement::Spread(
                                self.materialize_static_expression(expression),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Object(entries) => Expression::Object(
                entries
                    .iter()
                    .map(|entry| match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            crate::ir::hir::ObjectEntry::Data {
                                key: self.materialize_static_expression(key),
                                value: self.materialize_static_expression(value),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            crate::ir::hir::ObjectEntry::Getter {
                                key: self.materialize_static_expression(key),
                                getter: self.materialize_static_expression(getter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            crate::ir::hir::ObjectEntry::Setter {
                                key: self.materialize_static_expression(key),
                                setter: self.materialize_static_expression(setter),
                            }
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            crate::ir::hir::ObjectEntry::Spread(
                                self.materialize_static_expression(expression),
                            )
                        }
                    })
                    .collect(),
            ),
            Expression::Call { callee, arguments } => {
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && let Expression::String(property_name) = property.as_ref()
                    && matches!(property_name.as_str(), "toString" | "valueOf")
                    && let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_member_call_outcome_with_context(
                            object,
                            property_name,
                            self.current_user_function_name.as_deref(),
                        )
                {
                    return self.materialize_static_expression(&value);
                }
                if matches!(callee.as_ref(), Expression::Identifier(_))
                    && let Some(value) =
                        self.resolve_static_call_result_expression(callee, arguments)
                {
                    return self.materialize_static_expression(&value);
                }
                Expression::Call {
                    callee: Box::new(self.materialize_static_expression(callee)),
                    arguments: arguments
                        .iter()
                        .map(|argument| match argument {
                            CallArgument::Expression(expression) => CallArgument::Expression(
                                self.materialize_static_expression(expression),
                            ),
                            CallArgument::Spread(expression) => {
                                CallArgument::Spread(self.materialize_static_expression(expression))
                            }
                        })
                        .collect(),
                }
            }
            Expression::Assign { name, value } => Expression::Assign {
                name: name.clone(),
                value: Box::new(self.materialize_static_expression(value)),
            },
            Expression::AssignMember {
                object,
                property,
                value,
            } => Expression::AssignMember {
                object: Box::new(self.materialize_static_expression(object)),
                property: Box::new(self.materialize_static_expression(property)),
                value: Box::new(self.materialize_static_expression(value)),
            },
            Expression::AssignSuperMember { property, value } => Expression::AssignSuperMember {
                property: Box::new(self.materialize_static_expression(property)),
                value: Box::new(self.materialize_static_expression(value)),
            },
            Expression::Await(value) => {
                Expression::Await(Box::new(self.materialize_static_expression(value)))
            }
            Expression::EnumerateKeys(value) => {
                Expression::EnumerateKeys(Box::new(self.materialize_static_expression(value)))
            }
            Expression::GetIterator(value) => {
                Expression::GetIterator(Box::new(self.materialize_static_expression(value)))
            }
            Expression::IteratorClose(value) => {
                Expression::IteratorClose(Box::new(self.materialize_static_expression(value)))
            }
            Expression::New { callee, arguments } => Expression::New {
                callee: Box::new(self.materialize_static_expression(callee)),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => {
                            CallArgument::Expression(self.materialize_static_expression(expression))
                        }
                        CallArgument::Spread(expression) => {
                            CallArgument::Spread(self.materialize_static_expression(expression))
                        }
                    })
                    .collect(),
            },
            _ => expression.clone(),
        }
    }
}
