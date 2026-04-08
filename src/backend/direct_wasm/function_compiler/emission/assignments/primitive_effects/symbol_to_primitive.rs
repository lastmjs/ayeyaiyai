use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_effectful_symbol_to_primitive_for_operand(
        &mut self,
        expression: &Expression,
        default_argument: &Expression,
    ) -> DirectResult<SymbolToPrimitiveHandling> {
        let symbol_property = symbol_to_primitive_expression();
        if let Some(getter_binding) =
            self.resolve_member_getter_binding(expression, &symbol_property)
        {
            let getter_result_local = self.allocate_temp_local();
            if !self.emit_binding_call_result_to_local_with_explicit_this(
                &getter_binding,
                &[],
                expression,
                JS_TYPEOF_OBJECT_TAG,
                getter_result_local,
            )? {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            if self.function_binding_always_throws(&getter_binding) {
                return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
            }
            if let Some(return_expression) = self
                .resolve_function_binding_static_return_expression_with_call_frame(
                    &getter_binding,
                    &[],
                    expression,
                )
            {
                if let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                    &return_expression,
                    self.current_function_name(),
                ) {
                    if matches!(primitive, Expression::Null | Expression::Undefined) {
                        return Ok(SymbolToPrimitiveHandling::Handled);
                    }
                    self.emit_named_error_throw("TypeError")?;
                    return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
                }
                if let Some(return_binding) =
                    self.resolve_function_binding_from_expression(&return_expression)
                {
                    let return_result_local = self.allocate_temp_local();
                    if !self.emit_binding_call_result_to_local_with_explicit_this(
                        &return_binding,
                        std::slice::from_ref(default_argument),
                        expression,
                        JS_TYPEOF_OBJECT_TAG,
                        return_result_local,
                    )? {
                        return Ok(SymbolToPrimitiveHandling::NotHandled);
                    }
                    if self.function_binding_always_throws(&return_binding) {
                        return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
                    }
                    return Ok(SymbolToPrimitiveHandling::Handled);
                }
                self.emit_named_error_throw("TypeError")?;
                return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
            }
            if self.function_binding_defaults_to_undefined(&getter_binding) {
                return Ok(SymbolToPrimitiveHandling::Handled);
            }
        }

        if let Some(function_binding) = self
            .resolve_member_function_binding(expression, &symbol_property)
            .or_else(|| {
                self.resolve_object_binding_from_expression(expression)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(&object_binding, &symbol_property)
                            .and_then(|value| self.resolve_function_binding_from_expression(value))
                    })
            })
        {
            let result_local = self.allocate_temp_local();
            if !self.emit_binding_call_result_to_local_with_explicit_this(
                &function_binding,
                std::slice::from_ref(default_argument),
                expression,
                JS_TYPEOF_OBJECT_TAG,
                result_local,
            )? {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            if self.function_binding_always_throws(&function_binding) {
                return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
            }
            return Ok(SymbolToPrimitiveHandling::Handled);
        }

        if let Some(object_binding) = self.resolve_object_binding_from_expression(expression)
            && let Some(method_value) =
                object_binding_lookup_value(&object_binding, &symbol_property)
            && let Some(primitive) = self.resolve_static_primitive_expression_with_context(
                method_value,
                self.current_function_name(),
            )
        {
            if matches!(primitive, Expression::Null | Expression::Undefined) {
                return Ok(SymbolToPrimitiveHandling::NotHandled);
            }
            self.emit_named_error_throw("TypeError")?;
            return Ok(SymbolToPrimitiveHandling::AlwaysThrows);
        }

        Ok(SymbolToPrimitiveHandling::NotHandled)
    }

    pub(in crate::backend::direct_wasm) fn emit_effectful_symbol_to_primitive_addition(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<bool> {
        let default_argument = Expression::String("default".to_string());
        let left_handling =
            self.emit_effectful_symbol_to_primitive_for_operand(left, &default_argument)?;
        if left_handling == SymbolToPrimitiveHandling::AlwaysThrows {
            return Ok(true);
        }
        let right_handling =
            self.emit_effectful_symbol_to_primitive_for_operand(right, &default_argument)?;

        if left_handling == SymbolToPrimitiveHandling::NotHandled
            && right_handling == SymbolToPrimitiveHandling::NotHandled
        {
            return Ok(false);
        }

        if left_handling == SymbolToPrimitiveHandling::NotHandled {
            self.emit_numeric_expression(left)?;
            self.state.emission.output.instructions.push(0x1a);
        }
        if right_handling == SymbolToPrimitiveHandling::NotHandled {
            self.emit_numeric_expression(right)?;
            self.state.emission.output.instructions.push(0x1a);
        }

        self.push_i32_const(JS_NAN_TAG);
        Ok(true)
    }
}
