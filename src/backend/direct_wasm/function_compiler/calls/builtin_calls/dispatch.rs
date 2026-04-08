use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_builtin_call(
        &mut self,
        name: &str,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        let object_identifier = Expression::Identifier("Object".to_string());
        let array_identifier = Expression::Identifier("Array".to_string());
        if let Some(target_name) = parse_bound_function_prototype_call_builtin_name(name) {
            return self.emit_bound_function_prototype_call_builtin(target_name, arguments);
        }

        if matches!(
            name,
            "__assert" | "__assertSameValue" | "__assertNotSameValue"
        ) {
            return self.emit_assertion_builtin_call(name, arguments);
        }

        if name == "isNaN" {
            return self.emit_is_nan_call(arguments);
        }

        if name == "eval" {
            return self.emit_eval_call(arguments);
        }

        if name == TEST262_CREATE_REALM_BUILTIN {
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(true);
        }

        if self.emit_test262_realm_eval_call(name, arguments)? {
            return Ok(true);
        }

        if self.emit_function_constructor_builtin_call(name, arguments)? {
            return Ok(true);
        }

        if name == "String" {
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            let Some(value) = self.resolve_static_builtin_primitive_call_value(
                name,
                arguments,
                self.current_function_name(),
            ) else {
                return Ok(false);
            };
            self.emit_numeric_expression(&value)?;
            return Ok(true);
        }

        match name {
            "Array.isArray" => {
                return self.emit_array_is_array_call(
                    &array_identifier,
                    &Expression::String("isArray".to_string()),
                    arguments,
                );
            }
            "Object.create" => {
                return self.emit_object_create_call(
                    &object_identifier,
                    &Expression::String("create".to_string()),
                    arguments,
                );
            }
            "Object.getOwnPropertyDescriptor" => {
                return self.emit_object_get_own_property_descriptor_call(
                    &object_identifier,
                    &Expression::String("getOwnPropertyDescriptor".to_string()),
                    arguments,
                );
            }
            "Object.getOwnPropertyNames" => {
                return self.emit_object_array_builtin_call(
                    &object_identifier,
                    &Expression::String("getOwnPropertyNames".to_string()),
                    arguments,
                );
            }
            "Object.getOwnPropertySymbols" => {
                return self.emit_object_array_builtin_call(
                    &object_identifier,
                    &Expression::String("getOwnPropertySymbols".to_string()),
                    arguments,
                );
            }
            "Object.getPrototypeOf" => {
                return self.emit_object_get_prototype_of_call(
                    &object_identifier,
                    &Expression::String("getPrototypeOf".to_string()),
                    arguments,
                );
            }
            "Object.is" => {
                return self.emit_object_is_call(
                    &object_identifier,
                    &Expression::String("is".to_string()),
                    arguments,
                );
            }
            "Object.isExtensible" => {
                return self.emit_object_is_extensible_call(
                    &object_identifier,
                    &Expression::String("isExtensible".to_string()),
                    arguments,
                );
            }
            "Object.keys" => {
                return self.emit_object_array_builtin_call(
                    &object_identifier,
                    &Expression::String("keys".to_string()),
                    arguments,
                );
            }
            "Object.setPrototypeOf" => {
                return self.emit_object_set_prototype_of_call(
                    &object_identifier,
                    &Expression::String("setPrototypeOf".to_string()),
                    arguments,
                );
            }
            _ => {}
        }

        if let Some(native_error_value) = native_error_runtime_value(name) {
            for argument in arguments {
                match argument {
                    CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                        self.emit_numeric_expression(expression)?;
                        self.state.emission.output.instructions.push(0x1a);
                    }
                }
            }
            self.push_i32_const(native_error_value);
            return Ok(true);
        }

        let Some(result_tag) = (match name {
            "Promise.resolve" | "Promise.reject" => Some(JS_TYPEOF_OBJECT_TAG),
            "Number" => Some(JS_TYPEOF_NUMBER_TAG),
            "Boolean" => Some(JS_TYPEOF_BOOLEAN_TAG),
            "Object" | "Array" | "Date" | "RegExp" | "Map" | "Set" | "Error" | "EvalError"
            | "RangeError" | "ReferenceError" | "SyntaxError" | "TypeError" | "URIError"
            | "AggregateError" | "Promise" | "WeakRef" => Some(JS_TYPEOF_OBJECT_TAG),
            "BigInt" => Some(JS_TYPEOF_BIGINT_TAG),
            "Symbol" => Some(JS_TYPEOF_SYMBOL_TAG),
            _ => None,
        }) else {
            return Ok(false);
        };

        for argument in arguments {
            match argument {
                CallArgument::Expression(expression) => self.emit_numeric_expression(expression)?,
                CallArgument::Spread(expression) => {
                    self.emit_numeric_expression(expression)?;
                }
            }
            self.state.emission.output.instructions.push(0x1a);
        }
        self.push_i32_const(result_tag);
        Ok(true)
    }
}
