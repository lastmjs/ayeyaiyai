use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_array_buffer_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<(usize, usize)> {
        if let Expression::Identifier(name) = expression {
            let binding = self
                .state
                .speculation
                .static_semantics
                .local_resizable_array_buffer_binding(name)?;
            return Some((binding.values.len(), binding.max_length));
        }

        let (callee, arguments) = match expression {
            Expression::New { callee, arguments } => (callee.as_ref(), arguments.as_slice()),
            Expression::Call { callee, arguments } => {
                if !matches!(callee.as_ref(), Expression::Identifier(_)) {
                    return None;
                }
                let resolved = self.resolve_static_call_result_expression(callee, arguments)?;
                return self.resolve_array_buffer_binding_from_expression(&resolved);
            }
            _ => return None,
        };

        if !matches!(callee, Expression::Identifier(name) if name == "ArrayBuffer") {
            return None;
        }

        let length = extract_typed_array_element_count(match arguments.first()? {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => expression,
        })?;

        let max_length = arguments
            .get(1)
            .and_then(|argument| match argument {
                CallArgument::Expression(Expression::Object(entries))
                | CallArgument::Spread(Expression::Object(entries)) => {
                    entries.iter().find_map(|entry| {
                        let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                            return None;
                        };
                        if !matches!(key, Expression::String(name) if name == "maxByteLength") {
                            return None;
                        }
                        extract_typed_array_element_count(value)
                    })
                }
                _ => None,
            })
            .unwrap_or(length);

        Some((length, max_length))
    }

    pub(in crate::backend::direct_wasm) fn resolve_typed_array_view_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<TypedArrayViewBinding> {
        if let Expression::Identifier(name) = expression {
            return self
                .state
                .speculation
                .static_semantics
                .local_typed_array_view_binding(name)
                .cloned();
        }

        let arguments = match expression {
            Expression::New { arguments, .. } => arguments.as_slice(),
            Expression::Call { callee, arguments } => {
                if !matches!(callee.as_ref(), Expression::Identifier(_)) {
                    return None;
                }
                let resolved = self.resolve_static_call_result_expression(callee, arguments)?;
                return self.resolve_typed_array_view_binding_from_expression(&resolved);
            }
            _ => return None,
        };
        let buffer_expression = match arguments.first()? {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => expression,
        };
        let Expression::Identifier(buffer_name) = buffer_expression else {
            return None;
        };
        if self
            .state
            .speculation
            .static_semantics
            .local_resizable_array_buffer_binding(buffer_name)
            .is_none()
        {
            return None;
        }

        let offset = arguments
            .get(1)
            .and_then(|argument| match argument {
                CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                    extract_typed_array_element_count(expression)
                }
            })
            .unwrap_or(0);
        let fixed_length = arguments.get(2).and_then(|argument| match argument {
            CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                extract_typed_array_element_count(expression)
            }
        });

        Some(TypedArrayViewBinding {
            buffer_name: buffer_name.clone(),
            offset,
            fixed_length,
        })
    }

    pub(in crate::backend::direct_wasm) fn typed_array_view_static_length(
        &self,
        view: &TypedArrayViewBinding,
    ) -> Option<usize> {
        let buffer = self
            .state
            .speculation
            .static_semantics
            .local_resizable_array_buffer_binding(&view.buffer_name)?;
        match view.fixed_length {
            Some(length) => {
                if view.offset + length > buffer.values.len() {
                    None
                } else {
                    Some(length)
                }
            }
            None => {
                if view.offset > buffer.values.len() {
                    None
                } else {
                    Some(buffer.values.len().saturating_sub(view.offset))
                }
            }
        }
    }

    pub(in crate::backend::direct_wasm) fn typed_array_view_static_values(
        &self,
        view: &TypedArrayViewBinding,
    ) -> Option<ArrayValueBinding> {
        let buffer = self
            .state
            .speculation
            .static_semantics
            .local_resizable_array_buffer_binding(&view.buffer_name)?;
        let length = self.typed_array_view_static_length(view)?;
        Some(ArrayValueBinding {
            values: buffer.values[view.offset..view.offset + length].to_vec(),
        })
    }
}
