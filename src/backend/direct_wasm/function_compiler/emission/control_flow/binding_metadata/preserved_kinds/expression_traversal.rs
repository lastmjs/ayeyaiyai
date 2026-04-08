use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_preserved_binding_kinds_from_expression(
        &self,
        invalidated_bindings: &HashSet<String>,
        preserved_kinds: &mut HashMap<String, StaticValueKind>,
        blocked_bindings: &mut HashSet<String>,
        expression: &Expression,
    ) {
        match expression {
            Expression::Update { name, .. } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    Some(StaticValueKind::Number),
                );
            }
            Expression::Assign { name, value } => {
                self.merge_preserved_binding_kind(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    name,
                    self.infer_value_kind(value),
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::Member { object, property } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    object,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
            }
            Expression::SuperMember { property } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    property,
                );
            }
            Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    value,
                );
            }
            Expression::Binary { left, right, .. } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    left,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    right,
                );
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    condition,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    then_expression,
                );
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    else_expression,
                );
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_preserved_binding_kinds_from_expression(
                        invalidated_bindings,
                        preserved_kinds,
                        blocked_bindings,
                        expression,
                    );
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.collect_preserved_binding_kinds_from_expression(
                    invalidated_bindings,
                    preserved_kinds,
                    blocked_bindings,
                    callee,
                );
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                expression,
                            );
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                expression,
                            );
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                key,
                            );
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                value,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                key,
                            );
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                getter,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                key,
                            );
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                setter,
                            );
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.collect_preserved_binding_kinds_from_expression(
                                invalidated_bindings,
                                preserved_kinds,
                                blocked_bindings,
                                expression,
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
            | Expression::Sent => {}
        }
    }
}
