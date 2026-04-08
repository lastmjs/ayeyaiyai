use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_verify_property_call(
        &mut self,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let [object_argument, property_argument, descriptor_argument, ..] = arguments else {
            return Ok(false);
        };
        let (
            CallArgument::Expression(object_expression),
            CallArgument::Expression(property_expression),
            CallArgument::Expression(descriptor_expression),
        ) = (object_argument, property_argument, descriptor_argument)
        else {
            return Ok(false);
        };

        let Some(descriptor) = resolve_property_descriptor_definition(descriptor_expression) else {
            return Ok(false);
        };
        let expected_value = descriptor.value.as_ref().map(|value| {
            let materialized = self.materialize_static_expression(value);
            match materialized {
                Expression::Identifier(name)
                    if name == "undefined" && self.is_unshadowed_builtin_identifier(&name) =>
                {
                    Expression::Undefined
                }
                _ => materialized,
            }
        });
        let expected_writable = descriptor.writable;
        let expected_enumerable = descriptor.enumerable;
        let expected_configurable = descriptor.configurable;
        let matches_value = |actual: &Expression| {
            expected_value
                .as_ref()
                .is_none_or(|expected| expected == actual)
        };
        let matches_bool =
            |actual: bool, expected: Option<bool>| expected.is_none_or(|value| value == actual);
        let matches_missing_bool = |expected: Option<bool>| expected.is_none();

        let direct_arguments = self.is_direct_arguments_object(object_expression);
        let arguments_binding = self.resolve_arguments_binding_from_expression(object_expression);
        let object_binding = self.resolve_object_binding_from_expression(object_expression);

        if direct_arguments && is_symbol_iterator_expression(property_expression) {
            if expected_value
                .as_ref()
                .is_some_and(|value| *value == arguments_symbol_iterator_expression())
                && matches_bool(true, expected_writable)
                && matches_bool(false, expected_enumerable)
                && matches_bool(true, expected_configurable)
            {
                for argument in arguments.iter().skip(3) {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                return Ok(true);
            }
            return Ok(false);
        }

        let property_name = match property_expression {
            Expression::String(text) => text.clone(),
            Expression::Number(value)
                if value.is_finite() && value.fract() == 0.0 && *value >= 0.0 =>
            {
                (*value as u64).to_string()
            }
            _ => return Ok(false),
        };
        let global_property_descriptor =
            (self.state.speculation.execution_context.top_level_function
                && matches!(object_expression, Expression::This))
            .then(|| {
                self.backend
                    .global_property_descriptor(&property_name)
                    .cloned()
            })
            .flatten();

        if direct_arguments
            && let Some(index) = canonical_array_index_from_property_name(&property_name)
        {
            let Some(slot) = self.state.parameters.arguments_slots.get(&index).cloned() else {
                return Ok(false);
            };
            let matches_descriptor = slot.state.present
                && matches_bool(slot.state.enumerable, expected_enumerable)
                && matches_bool(slot.state.configurable, expected_configurable)
                && if slot.state.is_accessor() {
                    matches_missing_bool(expected_writable) && expected_value.is_none()
                } else {
                    matches_bool(slot.state.writable, expected_writable)
                };
            if !matches_descriptor {
                return Ok(false);
            }
            if let Some(expected_value) = expected_value.as_ref() {
                let actual_local = self.allocate_temp_local();
                let expected_local = self.allocate_temp_local();
                self.emit_arguments_slot_read(index)?;
                self.push_local_set(actual_local);
                self.emit_numeric_expression(expected_value)?;
                self.push_local_set(expected_local);
                self.push_local_get(actual_local);
                self.push_local_get(expected_local);
                self.push_binary_op(BinaryOp::NotEqual)?;
                self.state.emission.output.instructions.push(0x04);
                self.state
                    .emission
                    .output
                    .instructions
                    .push(EMPTY_BLOCK_TYPE);
                self.push_control_frame();
                self.emit_error_throw()?;
                self.state.emission.output.instructions.push(0x0b);
                self.pop_control_frame();
            }
            for argument in arguments.iter().skip(3) {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        }

        let matches_property = if property_name == "length" {
            if direct_arguments {
                self.state
                    .speculation
                    .execution_context
                    .current_arguments_length_present
                    && self
                        .state
                        .speculation
                        .execution_context
                        .current_arguments_length_override
                        .as_ref()
                        .is_none_or(matches_value)
                    && matches_bool(true, expected_writable)
                    && matches_bool(false, expected_enumerable)
                    && matches_bool(true, expected_configurable)
            } else if let Some(arguments_binding) = arguments_binding.as_ref() {
                arguments_binding.length_present
                    && matches_value(&arguments_binding.length_value)
                    && matches_bool(true, expected_writable)
                    && matches_bool(false, expected_enumerable)
                    && matches_bool(true, expected_configurable)
            } else {
                false
            }
        } else if property_name == "callee" {
            let strict = if direct_arguments {
                Some(self.state.speculation.execution_context.strict_mode)
            } else {
                arguments_binding.as_ref().map(|binding| binding.strict)
            };
            if let Some(strict) = strict {
                if strict {
                    expected_value.is_none()
                        && matches_missing_bool(expected_writable)
                        && matches_bool(false, expected_enumerable)
                        && matches_bool(false, expected_configurable)
                } else {
                    let actual_value = if direct_arguments {
                        self.direct_arguments_callee_expression()
                    } else {
                        arguments_binding
                            .as_ref()
                            .and_then(|binding| binding.callee_value.clone())
                    };
                    let present = if direct_arguments {
                        self.state
                            .speculation
                            .execution_context
                            .current_arguments_callee_present
                    } else {
                        arguments_binding
                            .as_ref()
                            .is_some_and(|binding| binding.callee_present)
                    };
                    present
                        && actual_value.as_ref().is_none_or(matches_value)
                        && matches_bool(true, expected_writable)
                        && matches_bool(false, expected_enumerable)
                        && matches_bool(true, expected_configurable)
                }
            } else {
                false
            }
        } else if let Some(arguments_binding) = arguments_binding.as_ref() {
            if let Ok(index) = property_name.parse::<usize>() {
                arguments_binding
                    .values
                    .get(index)
                    .is_some_and(matches_value)
                    && matches_bool(true, expected_writable)
                    && matches_bool(true, expected_enumerable)
                    && matches_bool(true, expected_configurable)
            } else {
                false
            }
        } else if let Some(global_property_descriptor) = global_property_descriptor.as_ref() {
            matches_value(&global_property_descriptor.value)
                && match global_property_descriptor.writable {
                    Some(writable) => matches_bool(writable, expected_writable),
                    None => matches_missing_bool(expected_writable),
                }
                && matches_bool(global_property_descriptor.enumerable, expected_enumerable)
                && matches_bool(
                    global_property_descriptor.configurable,
                    expected_configurable,
                )
        } else if let Some(object_binding) = object_binding.as_ref() {
            let property = Expression::String(property_name.clone());
            object_binding_lookup_value(object_binding, &property).is_some_and(matches_value)
                && matches_bool(true, expected_writable)
                && matches_bool(
                    !object_binding
                        .non_enumerable_string_properties
                        .iter()
                        .any(|name| name == &property_name),
                    expected_enumerable,
                )
                && matches_bool(true, expected_configurable)
        } else {
            false
        };

        if !matches_property {
            return Ok(false);
        }

        for argument in arguments.iter().skip(3) {
            match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                    self.state.emission.output.instructions.push(0x1a);
                }
            }
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(true)
    }
}
