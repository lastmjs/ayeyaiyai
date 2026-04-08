use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_has_own_property_call_result(
        &self,
        expression: &Expression,
    ) -> Option<bool> {
        let Expression::Call { callee, arguments } = expression else {
            return None;
        };
        let (object, argument_property) = match callee.as_ref() {
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "hasOwnProperty") =>
            {
                let [CallArgument::Expression(argument_property)] = arguments.as_slice() else {
                    return None;
                };
                (object.as_ref(), argument_property)
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Expression::Member {
                    object: _target_object,
                    property: target_property,
                } = object.as_ref()
                else {
                    return None;
                };
                if !matches!(target_property.as_ref(), Expression::String(name) if name == "hasOwnProperty")
                {
                    return None;
                }
                let [
                    CallArgument::Expression(receiver),
                    CallArgument::Expression(argument_property),
                    ..,
                ] = arguments.as_slice()
                else {
                    return None;
                };
                (receiver, argument_property)
            }
            _ => return None,
        };

        if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
            return Some(
                matches!(argument_property, Expression::String(property_name) if property_name == "length")
                    || argument_index_from_expression(argument_property).is_some_and(|index| {
                        array_binding
                            .values
                            .get(index as usize)
                            .is_some_and(|value| value.is_some())
                    }),
            );
        }

        if self.is_direct_arguments_object(object) {
            return match argument_property {
                Expression::String(property_name) => match property_name.as_str() {
                    "callee" | "length" => Some(self.direct_arguments_has_property(property_name)),
                    _ => canonical_array_index_from_property_name(property_name)
                        .map(|index| self.state.parameters.arguments_slots.contains_key(&index)),
                },
                _ => None,
            };
        }

        if let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(object) {
            return match argument_property {
                Expression::String(property_name) => Some(match property_name.as_str() {
                    "callee" => arguments_binding.callee_present,
                    "length" => arguments_binding.length_present,
                    _ => property_name
                        .parse::<usize>()
                        .ok()
                        .is_some_and(|index| index < arguments_binding.values.len()),
                }),
                _ => None,
            };
        }

        if self.resolve_user_function_from_expression(object).is_some() {
            return match argument_property {
                Expression::String(property_name)
                    if property_name == "caller" || property_name == "arguments" =>
                {
                    Some(false)
                }
                _ => None,
            };
        }

        if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
            return Some(
                self.resolve_object_binding_property_value(&object_binding, argument_property)
                    .is_some(),
            );
        }

        None
    }
}
