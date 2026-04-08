use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_has_own_property_call(
        &mut self,
        object: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Expression::Member {
            object: _target_object,
            property: target_property,
        } = object
        else {
            return Ok(false);
        };
        if !matches!(target_property.as_ref(), Expression::String(name) if name == "hasOwnProperty")
        {
            return Ok(false);
        }
        let [
            CallArgument::Expression(receiver),
            CallArgument::Expression(argument_property),
            rest @ ..,
        ] = arguments
        else {
            return Ok(false);
        };

        let result = if let Some(array_binding) =
            self.resolve_array_binding_from_expression(receiver)
        {
            Some(
                matches!(argument_property, Expression::String(property_name) if property_name == "length")
                    || argument_index_from_expression(argument_property).is_some_and(|index| {
                        array_binding
                            .values
                            .get(index as usize)
                            .is_some_and(|value| value.is_some())
                    }),
            )
        } else if self.is_direct_arguments_object(receiver) {
            match argument_property {
                Expression::String(property_name) => match property_name.as_str() {
                    "callee" | "length" => Some(self.direct_arguments_has_property(property_name)),
                    _ => canonical_array_index_from_property_name(property_name)
                        .map(|index| self.state.parameters.arguments_slots.contains_key(&index)),
                },
                _ => None,
            }
        } else if let Some(arguments_binding) =
            self.resolve_arguments_binding_from_expression(receiver)
        {
            match argument_property {
                Expression::String(property_name) => Some(match property_name.as_str() {
                    "callee" => arguments_binding.callee_present,
                    "length" => arguments_binding.length_present,
                    _ => property_name
                        .parse::<usize>()
                        .ok()
                        .is_some_and(|index| index < arguments_binding.values.len()),
                }),
                _ => None,
            }
        } else if let Some(object_binding) = self.resolve_object_binding_from_expression(receiver) {
            Some(
                self.resolve_object_binding_property_value(&object_binding, argument_property)
                    .is_some(),
            )
        } else if self
            .resolve_user_function_from_expression(receiver)
            .is_some()
        {
            match argument_property {
                Expression::String(property_name)
                    if property_name == "caller" || property_name == "arguments" =>
                {
                    Some(false)
                }
                _ => None,
            }
        } else {
            None
        };
        let Some(has_property) = result else {
            return Ok(false);
        };

        self.emit_numeric_expression(object)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(receiver)?;
        self.state.emission.output.instructions.push(0x1a);
        self.emit_numeric_expression(argument_property)?;
        self.state.emission.output.instructions.push(0x1a);
        for argument in rest {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }
        self.push_i32_const(if has_property { 1 } else { 0 });
        Ok(true)
    }
}
