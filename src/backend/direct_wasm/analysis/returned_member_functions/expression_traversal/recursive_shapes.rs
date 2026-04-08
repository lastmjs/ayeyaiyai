use super::super::*;

pub(super) fn collect_recursive_returned_member_function_bindings(
    expression: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &mut HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) {
    match expression {
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            super::collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            super::collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
            super::collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            super::collect_returned_member_function_bindings_from_expression(
                expression,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Binary { left, right, .. } => {
            super::collect_returned_member_function_bindings_from_expression(
                left,
                returned_identifier,
                function_names,
                bindings,
            );
            super::collect_returned_member_function_bindings_from_expression(
                right,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            super::collect_returned_member_function_bindings_from_expression(
                condition,
                returned_identifier,
                function_names,
                bindings,
            );
            super::collect_returned_member_function_bindings_from_expression(
                then_expression,
                returned_identifier,
                function_names,
                bindings,
            );
            super::collect_returned_member_function_bindings_from_expression(
                else_expression,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                super::collect_returned_member_function_bindings_from_expression(
                    expression,
                    returned_identifier,
                    function_names,
                    bindings,
                );
            }
        }
        Expression::Member { object, property } => {
            super::collect_returned_member_function_bindings_from_expression(
                object,
                returned_identifier,
                function_names,
                bindings,
            );
            super::collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            super::collect_returned_member_function_bindings_from_expression(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::SuperMember { property } => {
            super::collect_returned_member_function_bindings_from_expression(
                property,
                returned_identifier,
                function_names,
                bindings,
            );
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    crate::ir::hir::ArrayElement::Expression(expression)
                    | crate::ir::hir::ArrayElement::Spread(expression) => {
                        super::collect_returned_member_function_bindings_from_expression(
                            expression,
                            returned_identifier,
                            function_names,
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
                        super::collect_returned_member_function_bindings_from_expression(
                            key,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                        super::collect_returned_member_function_bindings_from_expression(
                            value,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Getter { key, getter }
                    | crate::ir::hir::ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        super::collect_returned_member_function_bindings_from_expression(
                            key,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                        super::collect_returned_member_function_bindings_from_expression(
                            getter,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                    crate::ir::hir::ObjectEntry::Spread(value) => {
                        super::collect_returned_member_function_bindings_from_expression(
                            value,
                            returned_identifier,
                            function_names,
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
        | Expression::Update { .. }
        | Expression::Call { .. }
        | Expression::SuperCall { .. }
        | Expression::New { .. } => {}
    }
}
