use super::*;

pub(super) fn collect_iterator_close_names_from_expression(
    expression: &Expression,
    names: &mut Vec<String>,
) {
    match expression {
        Expression::IteratorClose(value) => {
            if let Expression::Identifier(name) = value.as_ref() {
                names.push(name.clone());
            }
            collect_iterator_close_names_from_expression(value, names);
        }
        Expression::Member { object, property } => {
            collect_iterator_close_names_from_expression(object, names);
            collect_iterator_close_names_from_expression(property, names);
        }
        Expression::SuperMember { property } => {
            collect_iterator_close_names_from_expression(property, names);
        }
        Expression::Assign { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::GetIterator(value)
        | Expression::Unary {
            expression: value, ..
        } => collect_iterator_close_names_from_expression(value, names),
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            collect_iterator_close_names_from_expression(object, names);
            collect_iterator_close_names_from_expression(property, names);
            collect_iterator_close_names_from_expression(value, names);
        }
        Expression::AssignSuperMember { property, value } => {
            collect_iterator_close_names_from_expression(property, names);
            collect_iterator_close_names_from_expression(value, names);
        }
        Expression::Binary { left, right, .. } => {
            collect_iterator_close_names_from_expression(left, names);
            collect_iterator_close_names_from_expression(right, names);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_iterator_close_names_from_expression(condition, names);
            collect_iterator_close_names_from_expression(then_expression, names);
            collect_iterator_close_names_from_expression(else_expression, names);
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_iterator_close_names_from_expression(expression, names);
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_iterator_close_names_from_expression(callee, names);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_iterator_close_names_from_expression(expression, names);
                    }
                }
            }
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        collect_iterator_close_names_from_expression(expression, names);
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        collect_iterator_close_names_from_expression(key, names);
                        collect_iterator_close_names_from_expression(value, names);
                    }
                    ObjectEntry::Getter { key, getter } => {
                        collect_iterator_close_names_from_expression(key, names);
                        collect_iterator_close_names_from_expression(getter, names);
                    }
                    ObjectEntry::Setter { key, setter } => {
                        collect_iterator_close_names_from_expression(key, names);
                        collect_iterator_close_names_from_expression(setter, names);
                    }
                    ObjectEntry::Spread(expression) => {
                        collect_iterator_close_names_from_expression(expression, names);
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
