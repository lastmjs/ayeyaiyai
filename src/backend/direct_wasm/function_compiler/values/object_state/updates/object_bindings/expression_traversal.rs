use super::super::super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_object_binding_from_expression(
        &mut self,
        expression: &Expression,
    ) {
        match expression {
            Expression::Member { object, property } => {
                self.update_object_binding_from_expression(object);
                self.update_object_binding_from_expression(property);
            }
            Expression::SuperMember { property } => {
                self.update_object_binding_from_expression(property);
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.update_object_binding_from_expression(value),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.update_object_binding_from_expression(object);
                self.update_object_binding_from_expression(property);
                self.update_object_binding_from_expression(value);
            }
            Expression::AssignSuperMember { property, value } => {
                self.update_object_binding_from_expression(property);
                self.update_object_binding_from_expression(value);
            }
            Expression::Binary { left, right, .. } => {
                self.update_object_binding_from_expression(left);
                self.update_object_binding_from_expression(right);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.update_object_binding_from_expression(condition);
                self.update_object_binding_from_expression(then_expression);
                self.update_object_binding_from_expression(else_expression);
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.update_object_binding_from_expression(expression);
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.update_object_binding_from_expression(callee);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.update_object_binding_from_expression(expression);
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.update_object_binding_from_expression(expression);
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.update_object_binding_from_expression(key);
                            self.update_object_binding_from_expression(value);
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.update_object_binding_from_expression(key);
                            self.update_object_binding_from_expression(getter);
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.update_object_binding_from_expression(key);
                            self.update_object_binding_from_expression(setter);
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.update_object_binding_from_expression(expression);
                        }
                    }
                }
            }
            _ => {}
        }

        self.apply_builtin_object_binding_updates(expression);
    }
}
