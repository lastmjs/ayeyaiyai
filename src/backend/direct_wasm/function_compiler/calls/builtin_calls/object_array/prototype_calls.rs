use super::*;

impl<'a> FunctionCompiler<'a> {
    fn resolve_bound_function_prototype_call_descriptor(
        &self,
        receiver: &Expression,
        property: &Expression,
    ) -> Option<PropertyDescriptorBinding> {
        self.resolve_descriptor_binding_from_expression(&Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(Expression::Identifier("Object".to_string())),
                property: Box::new(Expression::String("getOwnPropertyDescriptor".to_string())),
            }),
            arguments: vec![
                CallArgument::Expression(receiver.clone()),
                CallArgument::Expression(property.clone()),
            ],
        })
    }

    fn resolve_bound_array_join_value(
        &self,
        receiver: &Expression,
        arguments: &[CallArgument],
    ) -> Option<Expression> {
        let array_binding = self.resolve_array_binding_from_expression(receiver)?;
        let separator = match arguments.first() {
            Some(CallArgument::Expression(expression) | CallArgument::Spread(expression)) => {
                self.resolve_static_string_concat_value(expression, self.current_function_name())?
            }
            None => ",".to_string(),
        };
        let mut parts = Vec::with_capacity(array_binding.values.len());
        for value in &array_binding.values {
            let Some(value) = value else {
                parts.push(String::new());
                continue;
            };
            let materialized = self
                .resolve_static_primitive_expression_with_context(
                    value,
                    self.current_function_name(),
                )
                .unwrap_or_else(|| self.materialize_static_expression(value));
            let text = match materialized {
                Expression::Undefined | Expression::Null => String::new(),
                _ => self.resolve_static_string_concat_value(
                    &materialized,
                    self.current_function_name(),
                )?,
            };
            parts.push(text);
        }
        Some(Expression::String(parts.join(&separator)))
    }

    pub(in crate::backend::direct_wasm) fn emit_bound_function_prototype_call_builtin(
        &mut self,
        target_name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let Some(CallArgument::Expression(receiver) | CallArgument::Spread(receiver)) =
            arguments.first()
        else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(true);
        };
        let target_arguments = &arguments[1..];

        match target_name {
            "Array.prototype.push" => {
                return self.emit_tracked_array_push_call(receiver, target_arguments);
            }
            "Array.prototype.join" => {
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                if let Some(value) = self.resolve_bound_array_join_value(receiver, target_arguments)
                {
                    self.emit_numeric_expression(&value)?;
                    return Ok(true);
                }
                return Ok(false);
            }
            "Object.prototype.hasOwnProperty" => {
                let Some(CallArgument::Expression(property) | CallArgument::Spread(property)) =
                    target_arguments.first()
                else {
                    self.push_i32_const(0);
                    return Ok(true);
                };
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(
                    self.resolve_bound_function_prototype_call_descriptor(receiver, property)
                        .is_some() as i32,
                );
                return Ok(true);
            }
            "Object.prototype.propertyIsEnumerable" => {
                let Some(CallArgument::Expression(property) | CallArgument::Spread(property)) =
                    target_arguments.first()
                else {
                    self.push_i32_const(0);
                    return Ok(true);
                };
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.state.emission.output.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(
                    self.resolve_bound_function_prototype_call_descriptor(receiver, property)
                        .is_some_and(|descriptor| descriptor.enumerable) as i32,
                );
                return Ok(true);
            }
            _ => {}
        }

        Ok(false)
    }
}
