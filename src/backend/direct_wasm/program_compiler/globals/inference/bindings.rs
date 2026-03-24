use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn infer_global_arguments_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArgumentsValueBinding> {
        match expression {
            Expression::Identifier(name) => self.global_arguments_bindings.get(name).cloned(),
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                if !user_function.returns_arguments_object {
                    return None;
                }
                Some(ArgumentsValueBinding::for_user_function(
                    user_function,
                    expand_static_call_arguments(arguments, &self.global_array_bindings),
                ))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn infer_global_array_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        match expression {
            Expression::Identifier(name) => self.global_array_bindings.get(name).cloned(),
            Expression::EnumerateKeys(value) => self.infer_enumerated_keys_binding(value),
            Expression::Call { callee, arguments } => {
                if let Some(binding) =
                    self.infer_global_builtin_array_call_binding(callee, arguments)
                {
                    return Some(binding);
                }
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.infer_enumerated_keys_binding(argument)
            }
            Expression::New { callee, arguments } => {
                let Expression::Identifier(name) = callee.as_ref() else {
                    return None;
                };
                let user_function = if let Some(LocalFunctionBinding::User(function_name)) =
                    self.global_function_bindings.get(name)
                {
                    self.user_function_map.get(function_name)
                } else if is_internal_user_function_identifier(name) {
                    self.user_function_map.get(name)
                } else {
                    None
                }?;
                let param_index = user_function.enumerated_keys_param_index?;
                let argument = match arguments.get(param_index) {
                    Some(CallArgument::Expression(expression))
                    | Some(CallArgument::Spread(expression)) => expression,
                    None => return Some(ArrayValueBinding { values: Vec::new() }),
                };
                self.infer_enumerated_keys_binding(argument)
            }
            Expression::Array(elements) => {
                let mut values = Vec::new();
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression) => {
                            values.push(Some(self.materialize_global_expression(expression)));
                        }
                        crate::ir::hir::ArrayElement::Spread(expression) => {
                            if let Some(binding) = self.infer_global_array_binding(expression) {
                                values.extend(binding.values);
                            } else {
                                values.push(Some(self.materialize_global_expression(expression)));
                            }
                        }
                    }
                }
                Some(ArrayValueBinding { values })
            }
            _ => None,
        }
    }
}
