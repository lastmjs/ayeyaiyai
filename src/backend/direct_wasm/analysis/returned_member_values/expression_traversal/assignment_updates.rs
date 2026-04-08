use super::super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_value_bindings_from_expression(
    expression: &Expression,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
    bindings: &mut HashMap<String, Expression>,
) {
    match expression {
        Expression::AssignMember {
            object,
            property,
            value,
        } => {
            if matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier)
            {
                if let Expression::String(property_name) = property.as_ref() {
                    bindings.insert(property_name.clone(), (**value).clone());
                }
            }
            super::recursive_shapes::collect_recursive_returned_member_value_bindings(
                object,
                returned_identifier,
                local_aliases,
                bindings,
            );
            super::recursive_shapes::collect_recursive_returned_member_value_bindings(
                property,
                returned_identifier,
                local_aliases,
                bindings,
            );
            super::recursive_shapes::collect_recursive_returned_member_value_bindings(
                value,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            super::recursive_shapes::collect_recursive_returned_member_value_bindings(
                callee,
                returned_identifier,
                local_aliases,
                bindings,
            );
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        super::recursive_shapes::collect_recursive_returned_member_value_bindings(
                            expression,
                            returned_identifier,
                            local_aliases,
                            bindings,
                        );
                    }
                }
            }

            super::define_property::collect_define_property_returned_member_value_binding(
                callee,
                arguments,
                returned_identifier,
                local_aliases,
                bindings,
            );
        }
        _ => super::recursive_shapes::collect_recursive_returned_member_value_bindings(
            expression,
            returned_identifier,
            local_aliases,
            bindings,
        ),
    }
}
