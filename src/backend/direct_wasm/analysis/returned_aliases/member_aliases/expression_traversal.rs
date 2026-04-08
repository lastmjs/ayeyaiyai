use super::super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_local_aliases_from_expression(
    expression: &Expression,
    aliases: &mut HashMap<String, Expression>,
) {
    match expression {
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_returned_member_local_aliases_from_expression(callee, aliases);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_returned_member_local_aliases_from_expression(expression, aliases);
                    }
                }
            }
        }
        Expression::Unary { expression, .. }
        | Expression::Await(expression)
        | Expression::EnumerateKeys(expression)
        | Expression::GetIterator(expression)
        | Expression::IteratorClose(expression) => {
            collect_returned_member_local_aliases_from_expression(expression, aliases);
        }
        Expression::Binary { left, right, .. } => {
            collect_returned_member_local_aliases_from_expression(left, aliases);
            collect_returned_member_local_aliases_from_expression(right, aliases);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_returned_member_local_aliases_from_expression(condition, aliases);
            collect_returned_member_local_aliases_from_expression(then_expression, aliases);
            collect_returned_member_local_aliases_from_expression(else_expression, aliases);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_returned_member_local_aliases_from_expression(expression, aliases);
            }
        }
        Expression::Member { object, property } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
        }
        Expression::Assign { value, .. } | Expression::AssignSuperMember { value, .. } => {
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_returned_member_local_aliases_from_expression(object, aliases);
            collect_returned_member_local_aliases_from_expression(property, aliases);
            collect_returned_member_local_aliases_from_expression(value, aliases);
        }
        Expression::SuperMember { property } => {
            collect_returned_member_local_aliases_from_expression(property, aliases);
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        collect_returned_member_local_aliases_from_expression(expression, aliases);
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(value, aliases);
                    }
                    ObjectEntry::Getter { key, getter } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(getter, aliases);
                    }
                    ObjectEntry::Setter { key, setter } => {
                        collect_returned_member_local_aliases_from_expression(key, aliases);
                        collect_returned_member_local_aliases_from_expression(setter, aliases);
                    }
                    ObjectEntry::Spread(value) => {
                        collect_returned_member_local_aliases_from_expression(value, aliases);
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
    }
}
