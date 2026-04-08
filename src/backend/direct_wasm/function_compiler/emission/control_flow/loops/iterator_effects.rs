use super::*;

impl<'a> FunctionCompiler<'a> {
    fn collect_effectful_iterator_assigned_binding_names_from_expression(
        &self,
        expression: &Expression,
        names: &mut HashSet<String>,
    ) {
        match expression {
            Expression::Call { callee, arguments } => {
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && matches!(property.as_ref(), Expression::String(name) if name == "next")
                    && let Some(IteratorSourceKind::SimpleGenerator {
                        steps,
                        completion_effects,
                        ..
                    }) = self.resolve_iterator_source_kind(object)
                {
                    for step in &steps {
                        for effect in &step.effects {
                            collect_assigned_binding_names_from_statement(effect, names);
                        }
                    }
                    for effect in &completion_effects {
                        collect_assigned_binding_names_from_statement(effect, names);
                    }
                }
                collect_assigned_binding_names_from_expression(callee, names);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                argument, names,
                            );
                        }
                    }
                }
            }
            Expression::Member { object, property } => {
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    object, names,
                );
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    property, names,
                );
            }
            Expression::SuperMember { property } => {
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    property, names,
                );
            }
            Expression::Assign { value, .. }
            | Expression::AssignSuperMember { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => {
                self.collect_effectful_iterator_assigned_binding_names_from_expression(value, names)
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    object, names,
                );
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    property, names,
                );
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    value, names,
                );
            }
            Expression::Binary { left, right, .. } => {
                self.collect_effectful_iterator_assigned_binding_names_from_expression(left, names);
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    right, names,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    condition, names,
                );
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    then_expression,
                    names,
                );
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    else_expression,
                    names,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_effectful_iterator_assigned_binding_names_from_expression(
                        expression, names,
                    );
                }
            }
            Expression::New { callee, arguments } | Expression::SuperCall { callee, arguments } => {
                self.collect_effectful_iterator_assigned_binding_names_from_expression(
                    callee, names,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(argument) | CallArgument::Spread(argument) => {
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                argument, names,
                            );
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                expression, names,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        ObjectEntry::Data { key, value } => {
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                key, names,
                            );
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                value, names,
                            );
                        }
                        ObjectEntry::Getter { key, getter } => {
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                key, names,
                            );
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                getter, names,
                            );
                        }
                        ObjectEntry::Setter { key, setter } => {
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                key, names,
                            );
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                setter, names,
                            );
                        }
                        ObjectEntry::Spread(expression) => {
                            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                                expression, names,
                            );
                        }
                    }
                }
            }
            Expression::Identifier(_)
            | Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent
            | Expression::Update { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_loop_assigned_binding_names_with_effectful_iterators(
        &self,
        condition: &Expression,
        break_hook: Option<&Expression>,
        body: &[Statement],
        init: Option<&[Statement]>,
        update: Option<&Expression>,
    ) -> HashSet<String> {
        let mut names =
            collect_loop_assigned_binding_names(condition, break_hook, body, init, update);
        self.collect_effectful_iterator_assigned_binding_names_from_expression(
            condition, &mut names,
        );
        if let Some(update) = update {
            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                update, &mut names,
            );
        }
        if let Some(break_hook) = break_hook {
            self.collect_effectful_iterator_assigned_binding_names_from_expression(
                break_hook, &mut names,
            );
        }
        names
    }
}
