use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_descriptor_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<PropertyDescriptorBinding> {
        match expression {
            Expression::Identifier(name) => self.local_descriptor_bindings.get(name).cloned(),
            Expression::Call { callee, arguments } => {
                let Expression::Member { object, property } = callee.as_ref() else {
                    return None;
                };
                if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
                    return None;
                }
                if !matches!(property.as_ref(), Expression::String(name) if name == "getOwnPropertyDescriptor")
                {
                    return None;
                }
                let [
                    CallArgument::Expression(target),
                    CallArgument::Expression(property_name),
                    ..,
                ] = arguments.as_slice()
                else {
                    return None;
                };
                let property_name = match property_name {
                    Expression::String(text) => text.as_str(),
                    _ => return None,
                };
                if property_name == "length" {
                    if self.is_direct_arguments_object(target) {
                        if !self.current_arguments_length_present {
                            return None;
                        }
                    } else if !self
                        .resolve_arguments_binding_from_expression(target)?
                        .length_present
                    {
                        return None;
                    }
                    return Some(PropertyDescriptorBinding {
                        value: if self.is_direct_arguments_object(target) {
                            self.current_arguments_length_override.clone().or(Some(
                                Expression::Member {
                                    object: Box::new(target.clone()),
                                    property: Box::new(Expression::String("length".to_string())),
                                },
                            ))
                        } else {
                            Some(
                                self.resolve_arguments_binding_from_expression(target)?
                                    .length_value
                                    .clone(),
                            )
                        },
                        configurable: true,
                        enumerable: false,
                        writable: Some(true),
                        has_get: false,
                        has_set: false,
                    });
                }
                if let Ok(index) = property_name.parse::<usize>() {
                    return Some(PropertyDescriptorBinding {
                        value: if self.is_direct_arguments_object(target) {
                            self.arguments_slots
                                .get(&(index as u32))
                                .filter(|slot| slot.state.present)
                                .map(|_| Expression::Undefined)
                                .or(Some(Expression::Undefined))
                        } else {
                            Some(
                                self.resolve_arguments_binding_from_expression(target)?
                                    .values
                                    .get(index)
                                    .cloned()
                                    .unwrap_or(Expression::Undefined),
                            )
                        },
                        configurable: true,
                        enumerable: true,
                        writable: Some(true),
                        has_get: false,
                        has_set: false,
                    });
                }
                if property_name == "callee" {
                    let strict = if self.is_direct_arguments_object(target) {
                        if !self.current_arguments_callee_present {
                            return None;
                        }
                        self.strict_mode
                    } else {
                        let binding = self.resolve_arguments_binding_from_expression(target)?;
                        if !binding.callee_present {
                            return None;
                        }
                        binding.strict
                    };
                    return Some(if strict {
                        PropertyDescriptorBinding {
                            value: None,
                            configurable: false,
                            enumerable: false,
                            writable: None,
                            has_get: true,
                            has_set: true,
                        }
                    } else {
                        PropertyDescriptorBinding {
                            value: if self.is_direct_arguments_object(target) {
                                self.direct_arguments_callee_expression()
                            } else {
                                self.resolve_arguments_binding_from_expression(target)?
                                    .callee_value
                                    .clone()
                            },
                            configurable: true,
                            enumerable: false,
                            writable: Some(true),
                            has_get: false,
                            has_set: false,
                        }
                    });
                }
                if self.top_level_function && matches!(target, Expression::This) {
                    return self
                        .resolve_top_level_global_property_descriptor_binding(property_name);
                }
                None
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_top_level_global_property_descriptor_binding(
        &self,
        property_name: &str,
    ) -> Option<PropertyDescriptorBinding> {
        if let Some(state) = self.module.global_property_descriptors.get(property_name) {
            return Some(PropertyDescriptorBinding {
                value: state.writable.map(|_| state.value.clone()),
                configurable: state.configurable,
                enumerable: state.enumerable,
                writable: state.writable,
                has_get: false,
                has_set: false,
            });
        }
        builtin_identifier_kind(property_name)?;
        Some(PropertyDescriptorBinding {
            value: Some(if property_name == "globalThis" {
                Expression::This
            } else {
                Expression::Member {
                    object: Box::new(Expression::This),
                    property: Box::new(Expression::String(property_name.to_string())),
                }
            }),
            configurable: builtin_identifier_delete_returns_true(property_name),
            enumerable: false,
            writable: Some(!matches!(property_name, "Infinity" | "NaN" | "undefined")),
            has_get: false,
            has_set: false,
        })
    }

    pub(in crate::backend::direct_wasm) fn update_local_descriptor_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(descriptor_binding) = self.resolve_descriptor_binding_from_expression(value)
        else {
            self.local_descriptor_bindings.remove(name);
            return;
        };
        self.local_descriptor_bindings
            .insert(name.to_string(), descriptor_binding);
        self.local_kinds
            .insert(name.to_string(), StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_global_property_descriptor_value(
        &mut self,
        name: &str,
        value_expression: &Expression,
    ) {
        let materialized = self
            .module
            .global_value_bindings
            .get(name)
            .cloned()
            .unwrap_or_else(|| self.materialize_static_expression(value_expression));
        if let Some(state) = self.module.global_property_descriptors.get_mut(name) {
            state.value = materialized;
        }
    }

    pub(in crate::backend::direct_wasm) fn ensure_global_property_descriptor_value(
        &mut self,
        name: &str,
        value_expression: &Expression,
        configurable: bool,
    ) {
        let materialized = self
            .module
            .global_value_bindings
            .get(name)
            .cloned()
            .unwrap_or_else(|| self.materialize_static_expression(value_expression));
        match self.module.global_property_descriptors.get_mut(name) {
            Some(state) => state.value = materialized,
            None => {
                self.module.global_property_descriptors.insert(
                    name.to_string(),
                    GlobalPropertyDescriptorState {
                        value: materialized,
                        writable: Some(true),
                        enumerable: true,
                        configurable,
                    },
                );
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn instantiate_eval_global_function_property_descriptor(
        &mut self,
        name: &str,
    ) {
        let value = Expression::Identifier(name.to_string());
        match self.module.global_property_descriptors.get_mut(name) {
            Some(state) if !state.configurable => {
                state.value = value;
            }
            Some(state) => {
                *state = GlobalPropertyDescriptorState {
                    value,
                    writable: Some(true),
                    enumerable: true,
                    configurable: true,
                };
            }
            None => {
                self.module.global_property_descriptors.insert(
                    name.to_string(),
                    GlobalPropertyDescriptorState {
                        value,
                        writable: Some(true),
                        enumerable: true,
                        configurable: true,
                    },
                );
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_value_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let snapshot_value = self
            .local_value_bindings
            .get(name)
            .or_else(|| self.module.global_value_bindings.get(name))
            .map(|snapshot| substitute_self_referential_binding_snapshot(value, name, snapshot))
            .unwrap_or_else(|| value.clone());
        let mut referenced_names = HashSet::new();
        collect_referenced_binding_names_from_expression(&snapshot_value, &mut referenced_names);
        if referenced_names.contains(name) {
            self.local_value_bindings.remove(name);
            return;
        }
        let materialized_value =
            if let Some(bigint) = self.resolve_static_bigint_value(&snapshot_value) {
                Expression::BigInt(bigint.to_string())
            } else {
                self.resolve_static_string_value(&snapshot_value)
                    .map(Expression::String)
                    .unwrap_or_else(|| self.materialize_static_expression(&snapshot_value))
            };
        self.local_value_bindings
            .insert(name.to_string(), materialized_value);
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_alias_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let mut current = expression;
        let mut visited = HashSet::new();
        loop {
            let Expression::Identifier(name) = current else {
                return Some(current.clone());
            };
            if self.with_scope_blocks_static_identifier_resolution(name) {
                return Some(current.clone());
            }
            if self.runtime_dynamic_bindings.contains(name) {
                return Some(current.clone());
            }
            if !visited.insert(name.clone()) {
                return None;
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && self.runtime_dynamic_bindings.contains(&resolved_name)
            {
                return Some(Expression::Identifier(resolved_name));
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && let Some(value) = self.local_value_bindings.get(&resolved_name)
            {
                current = value;
                continue;
            }
            if let Some(value) = self.local_value_bindings.get(name) {
                current = value;
                continue;
            }
            if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
                && let Some(value) = self.module.global_value_bindings.get(&hidden_name)
            {
                current = value;
                continue;
            }
            if let Some(value) = self.module.global_value_bindings.get(name) {
                current = value;
                continue;
            }
            return Some(current.clone());
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_symbol_identity_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let Expression::Identifier(name) = expression else {
            return None;
        };
        if self.lookup_identifier_kind(name) != Some(StaticValueKind::Symbol) {
            if let Some(resolved) = self.resolve_bound_alias_expression(expression)
                && !static_expression_matches(&resolved, expression)
            {
                if self.well_known_symbol_name(&resolved).is_some() {
                    return Some(resolved);
                }
                if let Expression::Identifier(resolved_name) = &resolved
                    && self.lookup_identifier_kind(resolved_name) == Some(StaticValueKind::Symbol)
                {
                    return Some(resolved);
                }
            }
            return None;
        }

        let mut current_name = name.clone();
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current_name.clone()) {
                return None;
            }
            let next = self
                .local_value_bindings
                .get(&current_name)
                .or_else(|| self.module.global_value_bindings.get(&current_name));
            match next {
                Some(Expression::Identifier(next_name))
                    if self.lookup_identifier_kind(next_name) == Some(StaticValueKind::Symbol) =>
                {
                    current_name = next_name.clone();
                }
                _ => return Some(Expression::Identifier(current_name)),
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_value_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let mut visited = HashSet::new();
        self.resolve_global_value_expression_with_visited(expression, &mut visited)
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_value_expression_with_visited(
        &self,
        expression: &Expression,
        visited: &mut HashSet<String>,
    ) -> Option<Expression> {
        let Expression::Identifier(name) = expression else {
            return Some(expression.clone());
        };
        if self.with_scope_blocks_static_identifier_resolution(name) {
            return None;
        }
        if !visited.insert(name.clone()) {
            return None;
        }
        let value = self.module.global_value_bindings.get(name)?.clone();
        self.resolve_global_identifiers_in_expression(&value, visited)
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_identifiers_in_expression(
        &self,
        expression: &Expression,
        visited: &mut HashSet<String>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name)
                if self.module.global_value_bindings.contains_key(name) =>
            {
                self.resolve_global_value_expression_with_visited(expression, visited)
            }
            Expression::Unary { op, expression } => Some(Expression::Unary {
                op: *op,
                expression: Box::new(
                    self.resolve_global_identifiers_in_expression(expression, visited)?,
                ),
            }),
            Expression::Binary { op, left, right } => Some(Expression::Binary {
                op: *op,
                left: Box::new(self.resolve_global_identifiers_in_expression(left, visited)?),
                right: Box::new(self.resolve_global_identifiers_in_expression(right, visited)?),
            }),
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => Some(Expression::Conditional {
                condition: Box::new(
                    self.resolve_global_identifiers_in_expression(condition, visited)?,
                ),
                then_expression: Box::new(
                    self.resolve_global_identifiers_in_expression(then_expression, visited)?,
                ),
                else_expression: Box::new(
                    self.resolve_global_identifiers_in_expression(else_expression, visited)?,
                ),
            }),
            Expression::Sequence(expressions) => Some(Expression::Sequence(
                expressions
                    .iter()
                    .map(|expression| {
                        self.resolve_global_identifiers_in_expression(expression, visited)
                    })
                    .collect::<Option<Vec<_>>>()?,
            )),
            Expression::Member { object, property } => Some(Expression::Member {
                object: Box::new(self.resolve_global_identifiers_in_expression(object, visited)?),
                property: Box::new(
                    self.resolve_global_identifiers_in_expression(property, visited)?,
                ),
            }),
            Expression::Call { callee, arguments } => Some(Expression::Call {
                callee: Box::new(self.resolve_global_identifiers_in_expression(callee, visited)?),
                arguments: arguments
                    .iter()
                    .map(|argument| match argument {
                        CallArgument::Expression(expression) => Some(CallArgument::Expression(
                            self.resolve_global_identifiers_in_expression(expression, visited)?,
                        )),
                        CallArgument::Spread(expression) => Some(CallArgument::Spread(
                            self.resolve_global_identifiers_in_expression(expression, visited)?,
                        )),
                    })
                    .collect::<Option<Vec<_>>>()?,
            }),
            _ => Some(expression.clone()),
        }
    }
}
