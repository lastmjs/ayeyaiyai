use super::super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_function_bindings_from_expression(
    expression: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &mut HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) {
    match expression {
        Expression::Call { callee, arguments }
        | Expression::SuperCall { callee, arguments }
        | Expression::New { callee, arguments } => {
            super::recursive_shapes::collect_recursive_returned_member_function_bindings(
                callee,
                returned_identifier,
                function_names,
                bindings,
            );
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        super::recursive_shapes::collect_recursive_returned_member_function_bindings(
                            expression,
                            returned_identifier,
                            function_names,
                            bindings,
                        );
                    }
                }
            }

            let Expression::Member { object, property } = callee.as_ref() else {
                return;
            };
            if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
                return;
            }
            if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
                return;
            }
            let [
                CallArgument::Expression(target),
                CallArgument::Expression(property),
                CallArgument::Expression(descriptor),
                ..,
            ] = arguments.as_slice()
            else {
                return;
            };
            let Some(key) =
                returned_member_function_binding_key(target, property, returned_identifier)
            else {
                return;
            };
            let Some(binding) = resolve_returned_member_function_binding_from_descriptor(
                descriptor,
                returned_identifier,
                function_names,
                bindings,
            ) else {
                bindings.remove(&key);
                return;
            };
            bindings.insert(key, binding);
        }
        _ => super::recursive_shapes::collect_recursive_returned_member_function_bindings(
            expression,
            returned_identifier,
            function_names,
            bindings,
        ),
    }
}
