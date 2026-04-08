use super::*;

pub(in crate::backend::direct_wasm) fn collect_arguments_usage_from_expression(
    expression: &Expression,
    indexed_slots: &mut HashSet<u32>,
    track_all_slots: &mut bool,
) {
    match expression {
        Expression::Member { object, property } => {
            if is_arguments_identifier(object) {
                if let Some(index) = argument_index_from_expression(property) {
                    indexed_slots.insert(index);
                } else {
                    *track_all_slots = true;
                }
            }
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
        }
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            if is_arguments_identifier(object) {
                if let Some(index) = argument_index_from_expression(property) {
                    indexed_slots.insert(index);
                } else {
                    *track_all_slots = true;
                }
            }
            collect_arguments_usage_from_expression(object, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::Assign { value, .. }
        | Expression::Await(value)
        | Expression::EnumerateKeys(value)
        | Expression::IteratorClose(value) => {
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::GetIterator(value) => {
            if is_arguments_identifier(value) {
                *track_all_slots = true;
            }
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
        }
        Expression::Unary { op, expression } => {
            if *op == UnaryOp::Delete
                && let Expression::Member { object, property } = expression.as_ref()
                && is_arguments_identifier(object)
            {
                if let Some(index) = argument_index_from_expression(property) {
                    indexed_slots.insert(index);
                } else {
                    *track_all_slots = true;
                }
            }
            collect_arguments_usage_from_expression(expression, indexed_slots, track_all_slots);
        }
        Expression::Binary { left, right, .. } => {
            collect_arguments_usage_from_expression(left, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(right, indexed_slots, track_all_slots);
        }
        Expression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => {
            collect_arguments_usage_from_expression(condition, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(
                then_expression,
                indexed_slots,
                track_all_slots,
            );
            collect_arguments_usage_from_expression(
                else_expression,
                indexed_slots,
                track_all_slots,
            );
        }
        Expression::Sequence(expressions) => {
            for expression in expressions {
                collect_arguments_usage_from_expression(expression, indexed_slots, track_all_slots);
            }
        }
        Expression::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Expression(expression) | ArrayElement::Spread(expression) => {
                        collect_arguments_usage_from_expression(
                            expression,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::Object(entries) => {
            for entry in entries {
                match entry {
                    ObjectEntry::Data { key, value } => {
                        collect_arguments_usage_from_expression(
                            key,
                            indexed_slots,
                            track_all_slots,
                        );
                        collect_arguments_usage_from_expression(
                            value,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                    ObjectEntry::Getter { key, getter }
                    | ObjectEntry::Setter {
                        key,
                        setter: getter,
                    } => {
                        collect_arguments_usage_from_expression(
                            key,
                            indexed_slots,
                            track_all_slots,
                        );
                        collect_arguments_usage_from_expression(
                            getter,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                    ObjectEntry::Spread(value) => {
                        collect_arguments_usage_from_expression(
                            value,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            collect_arguments_usage_from_expression(callee, indexed_slots, track_all_slots);
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        collect_arguments_usage_from_expression(
                            expression,
                            indexed_slots,
                            track_all_slots,
                        );
                    }
                }
            }
        }
        Expression::SuperMember { property } => {
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
        }
        Expression::AssignSuperMember { property, value } => {
            collect_arguments_usage_from_expression(property, indexed_slots, track_all_slots);
            collect_arguments_usage_from_expression(value, indexed_slots, track_all_slots);
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
