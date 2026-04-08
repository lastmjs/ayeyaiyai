use super::*;

thread_local! {
    static BOUND_ALIAS_RESOLUTION_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct BoundAliasResolutionGuard;

impl BoundAliasResolutionGuard {
    fn enter(expression: &Expression) -> Self {
        BOUND_ALIAS_RESOLUTION_DEPTH.with(|depth| {
            let next = depth.get() + 1;
            if next > 256 {
                panic!("bound alias resolution recursion overflow: expression={expression:?}");
            }
            depth.set(next);
        });
        Self
    }
}

impl Drop for BoundAliasResolutionGuard {
    fn drop(&mut self) {
        BOUND_ALIAS_RESOLUTION_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_bound_alias_expression_with_state(
        &self,
        expression: &Expression,
        environment: &StaticResolutionEnvironment,
    ) -> Option<Expression> {
        resolve_bound_alias_expression_in_environment(
            expression,
            environment,
            &|name| self.with_scope_blocks_static_identifier_resolution(name),
            &|name| {
                self.state
                    .runtime
                    .locals
                    .runtime_dynamic_bindings
                    .contains(name)
            },
            &|name, environment| environment.binding(name).cloned(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_bound_alias_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let _guard = BoundAliasResolutionGuard::enter(expression);
        let mut current = expression;
        let mut visited = HashSet::new();
        loop {
            let Expression::Identifier(name) = current else {
                return Some(current.clone());
            };
            if self.with_scope_blocks_static_identifier_resolution(name) {
                return Some(current.clone());
            }
            if self
                .state
                .runtime
                .locals
                .runtime_dynamic_bindings
                .contains(name)
            {
                return Some(current.clone());
            }
            if !visited.insert(name.clone()) {
                return None;
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && self
                    .state
                    .runtime
                    .locals
                    .runtime_dynamic_bindings
                    .contains(&resolved_name)
            {
                return Some(Expression::Identifier(resolved_name));
            }
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
                && let Some(value) = self
                    .state
                    .speculation
                    .static_semantics
                    .local_value_binding(&resolved_name)
            {
                current = value;
                continue;
            }
            if let Some(value) = self
                .state
                .speculation
                .static_semantics
                .local_value_binding(name)
            {
                current = value;
                continue;
            }
            if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
                && let Some(value) = self.global_value_binding(&hidden_name)
            {
                current = value;
                continue;
            }
            if let Some(value) = self.backend.global_value_binding(name) {
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
        if let Some((resolved_name, _)) = self.resolve_current_local_binding(name)
            && resolved_name != *name
            && let Some(resolved) =
                self.resolve_symbol_identity_expression(&Expression::Identifier(resolved_name))
        {
            return Some(resolved);
        }
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
            if let Some((resolved_name, _)) = self.resolve_current_local_binding(&current_name)
                && resolved_name != current_name
                && self.lookup_identifier_kind(&resolved_name) == Some(StaticValueKind::Symbol)
            {
                current_name = resolved_name;
                continue;
            }
            let next = self
                .state
                .speculation
                .static_semantics
                .local_value_binding(&current_name)
                .or_else(|| self.backend.global_value_binding(&current_name));
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
        let value = self.backend.global_value_binding(name)?.clone();
        self.resolve_global_identifiers_in_expression(&value, visited)
    }

    pub(in crate::backend::direct_wasm) fn resolve_global_identifiers_in_expression(
        &self,
        expression: &Expression,
        visited: &mut HashSet<String>,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) if self.backend.global_value_binding(name).is_some() => {
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
