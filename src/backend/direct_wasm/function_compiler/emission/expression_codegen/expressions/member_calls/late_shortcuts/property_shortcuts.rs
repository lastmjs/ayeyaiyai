use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn emit_property_member_call_shortcuts(
        &mut self,
        source_expression: &Expression,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if matches!(property, Expression::String(property_name) if property_name == "hasOwnProperty")
            && let [CallArgument::Expression(argument_property)] = arguments
        {
            if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
                let has_property = matches!(argument_property, Expression::String(property_name) if property_name == "length")
                    || argument_index_from_expression(argument_property).is_some_and(|index| {
                        array_binding
                            .values
                            .get(index as usize)
                            .is_some_and(|value| value.is_some())
                    });
                self.emit_numeric_expression(object)?;
                self.state.emission.output.instructions.push(0x1a);
                self.emit_numeric_expression(argument_property)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(if has_property { 1 } else { 0 });
                return Ok(true);
            }
            if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
                let has_property = self
                    .resolve_object_binding_property_value(&object_binding, argument_property)
                    .is_some();
                self.emit_numeric_expression(object)?;
                self.state.emission.output.instructions.push(0x1a);
                self.emit_numeric_expression(argument_property)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(if has_property { 1 } else { 0 });
                return Ok(true);
            }
            if self.resolve_user_function_from_expression(object).is_some()
                && let Expression::String(property_name) = argument_property
                && (property_name == "caller" || property_name == "arguments")
            {
                self.emit_numeric_expression(object)?;
                self.state.emission.output.instructions.push(0x1a);
                self.emit_numeric_expression(argument_property)?;
                self.state.emission.output.instructions.push(0x1a);
                self.push_i32_const(0);
                return Ok(true);
            }
        }

        if matches!(object, Expression::Identifier(name) if name == "Object")
            && matches!(property, Expression::String(property_name) if property_name == "defineProperty")
            && let [
                CallArgument::Expression(target),
                CallArgument::Expression(property_name_expression),
                CallArgument::Expression(descriptor),
                ..,
            ] = arguments
        {
            if self.is_direct_arguments_object(target)
                && let Some(index) = argument_index_from_expression(property_name_expression)
                && let Some(descriptor) = resolve_property_descriptor_definition(descriptor)
                && self.apply_direct_arguments_define_property(index, &descriptor)?
            {
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                return Ok(true);
            }

            self.emit_numeric_expression(target)?;
            self.state.emission.output.instructions.push(0x1a);
            self.emit_property_key_expression_effects(property_name_expression)?;
            self.emit_numeric_expression(descriptor)?;
            self.state.emission.output.instructions.push(0x1a);
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }

        if matches!(property, Expression::String(property_name) if property_name == "hasOwnProperty")
            && let [CallArgument::Expression(argument_property)] = arguments
        {
            let direct_arguments = self.is_direct_arguments_object(object);
            let arguments_binding = self.resolve_arguments_binding_from_expression(object);
            if direct_arguments && let Expression::String(owned_property_name) = argument_property {
                match owned_property_name.as_str() {
                    "callee" | "length" => {
                        self.push_i32_const(
                            if self.direct_arguments_has_property(owned_property_name) {
                                1
                            } else {
                                0
                            },
                        );
                        return Ok(true);
                    }
                    _ => {
                        if let Some(index) =
                            canonical_array_index_from_property_name(owned_property_name)
                        {
                            if let Some(slot) = self.state.parameters.arguments_slots.get(&index) {
                                self.push_local_get(slot.present_local);
                            } else {
                                self.push_i32_const(0);
                            }
                            return Ok(true);
                        }
                    }
                }
            }
            if let Some(arguments_binding) = arguments_binding.as_ref()
                && let Expression::String(owned_property_name) = argument_property
            {
                let has_property = match owned_property_name.as_str() {
                    "callee" => arguments_binding.callee_present,
                    "length" => arguments_binding.length_present,
                    _ => owned_property_name
                        .parse::<usize>()
                        .ok()
                        .is_some_and(|index| index < arguments_binding.values.len()),
                };
                self.push_i32_const(if has_property { 1 } else { 0 });
                return Ok(true);
            }
            if self.resolve_user_function_from_expression(object).is_some()
                && let Expression::String(owned_property_name) = argument_property
                && (owned_property_name == "caller" || owned_property_name == "arguments")
            {
                self.push_i32_const(0);
                return Ok(true);
            }
        }

        if let Expression::Identifier(name) = object {
            let resolved_name = self
                .resolve_current_local_binding(name)
                .map(|(resolved_name, _)| resolved_name)
                .unwrap_or_else(|| name.clone());
            if let Some(descriptor) = self
                .state
                .speculation
                .static_semantics
                .objects
                .local_descriptor_bindings
                .get(&resolved_name)
                && matches!(property, Expression::String(property_name) if property_name == "hasOwnProperty")
                && let [CallArgument::Expression(Expression::String(owned_property_name))] =
                    arguments
            {
                let has_property = match owned_property_name.as_str() {
                    "configurable" | "enumerable" => true,
                    "value" => descriptor.value.is_some(),
                    "writable" => descriptor.writable.is_some(),
                    "get" => descriptor.has_get,
                    "set" => descriptor.has_set,
                    _ => false,
                };
                self.push_i32_const(if has_property { 1 } else { 0 });
                return Ok(true);
            }
        }

        if self
            .resolve_descriptor_binding_from_expression(source_expression)
            .is_some()
        {
            self.emit_ignored_call_arguments(arguments)?;
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }

        Ok(false)
    }
}
