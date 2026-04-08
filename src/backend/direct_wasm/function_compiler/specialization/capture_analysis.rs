use super::*;

impl<'a> FunctionCompiler<'a> {
    fn static_with_scope_unscopables_blocks_for_specialization(
        &self,
        scope_object: &Expression,
        name: &str,
    ) -> Option<bool> {
        let unscopables_key = Expression::Member {
            object: Box::new(Expression::Identifier("Symbol".to_string())),
            property: Box::new(Expression::String("unscopables".to_string())),
        };
        if self
            .resolve_member_getter_binding(scope_object, &unscopables_key)
            .is_some()
        {
            return None;
        }
        let Some(scope_binding) = self.resolve_object_binding_from_expression(scope_object) else {
            return Some(false);
        };
        let Some(unscopables_value) = object_binding_lookup_value(&scope_binding, &unscopables_key)
        else {
            return Some(false);
        };
        let Some(unscopables_object) =
            self.resolve_object_binding_from_expression(unscopables_value)
        else {
            return Some(false);
        };
        let property = Expression::String(name.to_string());
        Some(
            object_binding_lookup_value(&unscopables_object, &property)
                .and_then(|value| self.resolve_static_boolean_expression(value))
                .unwrap_or(false),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_with_scope_binding_for_specialization(
        &self,
        name: &str,
    ) -> Option<Expression> {
        for scope_object in self.state.emission.lexical_scopes.with_scopes.iter().rev() {
            if self
                .resolve_proxy_binding_from_expression(scope_object)
                .is_some()
            {
                return None;
            }
            if !self.scope_object_has_binding_property(scope_object, name) {
                continue;
            }
            match self.static_with_scope_unscopables_blocks_for_specialization(scope_object, name) {
                Some(true) => continue,
                Some(false) => return Some(scope_object.clone()),
                None => return None,
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn collect_capture_bindings_from_expression(
        &self,
        expression: &Expression,
        bindings: &mut BTreeSet<String>,
    ) {
        match expression {
            Expression::Identifier(name) => {
                if self.resolve_current_local_binding(name).is_some()
                    || self
                        .resolve_with_scope_binding_for_specialization(name)
                        .is_some()
                {
                    bindings.insert(name.clone());
                }
            }
            Expression::Member { object, property } => {
                self.collect_capture_bindings_from_expression(object, bindings);
                self.collect_capture_bindings_from_expression(property, bindings);
            }
            Expression::SuperMember { property } => {
                self.collect_capture_bindings_from_expression(property, bindings);
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.collect_capture_bindings_from_expression(value, bindings),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_capture_bindings_from_expression(object, bindings);
                self.collect_capture_bindings_from_expression(property, bindings);
                self.collect_capture_bindings_from_expression(value, bindings);
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_capture_bindings_from_expression(property, bindings);
                self.collect_capture_bindings_from_expression(value, bindings);
            }
            Expression::Binary { left, right, .. } => {
                self.collect_capture_bindings_from_expression(left, bindings);
                self.collect_capture_bindings_from_expression(right, bindings);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_capture_bindings_from_expression(condition, bindings);
                self.collect_capture_bindings_from_expression(then_expression, bindings);
                self.collect_capture_bindings_from_expression(else_expression, bindings);
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_capture_bindings_from_expression(expression, bindings);
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.collect_capture_bindings_from_expression(callee, bindings);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_capture_bindings_from_expression(expression, bindings);
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.collect_capture_bindings_from_expression(expression, bindings);
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_capture_bindings_from_expression(key, bindings);
                            self.collect_capture_bindings_from_expression(value, bindings);
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_capture_bindings_from_expression(key, bindings);
                            self.collect_capture_bindings_from_expression(getter, bindings);
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_capture_bindings_from_expression(key, bindings);
                            self.collect_capture_bindings_from_expression(setter, bindings);
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.collect_capture_bindings_from_expression(expression, bindings);
                        }
                    }
                }
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent
            | Expression::Update { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_capture_bindings_from_summary(
        &self,
        summary: &InlineFunctionSummary,
    ) -> BTreeSet<String> {
        let mut bindings = BTreeSet::new();
        for effect in &summary.effects {
            match effect {
                InlineFunctionEffect::Assign { value, .. } => {
                    self.collect_capture_bindings_from_expression(value, &mut bindings);
                }
                InlineFunctionEffect::Update { .. } => {}
                InlineFunctionEffect::Expression(expression) => {
                    self.collect_capture_bindings_from_expression(expression, &mut bindings);
                }
            }
        }
        if let Some(return_value) = summary.return_value.as_ref() {
            self.collect_capture_bindings_from_expression(return_value, &mut bindings);
        }
        bindings
    }
}
