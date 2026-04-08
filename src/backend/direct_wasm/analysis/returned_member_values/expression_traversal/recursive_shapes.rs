use super::super::*;

pub(super) fn collect_recursive_returned_member_value_bindings(
    expression: &Expression,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
    bindings: &mut HashMap<String, Expression>,
) {
    match expression {
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            super::collect_returned_member_value_bindings_from_expression(
                expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Binary { left, right, .. } => {
            super::collect_returned_member_value_bindings_from_expression(
                left,
                returned_identifier,
                local_aliases,
                bindings,
            );
            super::collect_returned_member_value_bindings_from_expression(
                right,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            super::collect_returned_member_value_bindings_from_expression(
                condition,
                returned_identifier,
                local_aliases,
                bindings,
            );
            super::collect_returned_member_value_bindings_from_expression(
                then_expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
            super::collect_returned_member_value_bindings_from_expression(
                else_expression,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                super::collect_returned_member_value_bindings_from_expression(
                    expression,
                    returned_identifier,
                    local_aliases,
                    bindings,
                );
            }
        }
        Expression::Member { object, property } => {
            super::collect_returned_member_value_bindings_from_expression(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            super::collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            super::collect_returned_member_value_bindings_from_expression(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::SuperMember { property } => {
            super::collect_returned_member_value_bindings_from_expression(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        super::collect_returned_member_value_bindings_from_expression(
                            expression,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    crate::ir::hir::ObjectEntry::Data { key, value } => {
                        super::collect_returned_member_value_bindings_from_expression(
                            key,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                        super::collect_returned_member_value_bindings_from_expression(
                            value,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        super::collect_returned_member_value_bindings_from_expression(
                            key,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                        super::collect_returned_member_value_bindings_from_expression(
                            getter,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        super::collect_returned_member_value_bindings_from_expression(
                            value,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
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
        | Expression::Identifier(_)
        | Expression::This
        | Expression::Sent
        | Expression::Update { .. } => {}
        Expression::AssignMember { .. }
        | Expression::Call { .. }
        | Expression::SuperCall { .. }
        | Expression::New { .. } => {}
    }
}
