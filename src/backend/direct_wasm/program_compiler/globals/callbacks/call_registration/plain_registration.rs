use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_callback_bindings_for_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<LocalFunctionBinding>>>,
        array_bindings: &mut HashMap<String, HashMap<String, Option<ArrayValueBinding>>>,
        object_bindings: &mut HashMap<String, HashMap<String, Option<ObjectValueBinding>>>,
    ) {
        let (called_function_name, call_arguments) = match callee {
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "call") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    self.expanded_global_static_call_arguments(arguments)
                        .into_iter()
                        .skip(1)
                        .collect::<Vec<_>>(),
                )
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "apply") =>
            {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(object, aliases)
                else {
                    return;
                };
                let expanded_arguments = self.expanded_global_static_call_arguments(arguments);
                let apply_expression = expanded_arguments
                    .get(1)
                    .cloned()
                    .unwrap_or(Expression::Undefined);
                let Some(call_arguments) =
                    self.expand_apply_parameter_call_arguments_from_expression(&apply_expression)
                else {
                    return;
                };
                (called_function_name, call_arguments)
            }
            _ => {
                let Some(LocalFunctionBinding::User(called_function_name)) =
                    self.resolve_function_binding_from_expression_with_aliases(callee, aliases)
                else {
                    return;
                };
                (
                    called_function_name,
                    self.expanded_global_static_call_arguments(arguments),
                )
            }
        };
        let Some(user_function) = self.user_function(&called_function_name) else {
            return;
        };
        let Some(parameter_bindings) = bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_array_bindings) = array_bindings.get_mut(&called_function_name) else {
            return;
        };
        let Some(parameter_object_bindings) = object_bindings.get_mut(&called_function_name) else {
            return;
        };

        let mut register_candidate =
            |param_name: &str, candidate: Option<LocalFunctionBinding>| match candidate {
                None => {
                    parameter_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_object_candidate =
            |param_name: &str, candidate: Option<ObjectValueBinding>| match candidate {
                None => {
                    parameter_object_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_object_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_object_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_object_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };
        let mut register_array_candidate =
            |param_name: &str, candidate: Option<ArrayValueBinding>| match candidate {
                None => {
                    parameter_array_bindings.insert(param_name.to_string(), None);
                }
                Some(binding) => match parameter_array_bindings.get(param_name) {
                    Some(None) => {}
                    Some(Some(existing)) if *existing == binding => {}
                    Some(Some(_)) => {
                        parameter_array_bindings.insert(param_name.to_string(), None);
                    }
                    None => {
                        parameter_array_bindings.insert(param_name.to_string(), Some(binding));
                    }
                },
            };

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            register_candidate(
                param_name,
                self.resolve_function_binding_from_expression_with_aliases(argument, aliases),
            );
            register_array_candidate(param_name, self.infer_global_array_binding(argument));
            let global_bindings = self.snapshot_global_binding_environment();
            let materialized_argument = self
                .materialize_global_expression_with_state(
                    argument,
                    &HashMap::new(),
                    &global_bindings.value_bindings,
                    &global_bindings.object_bindings,
                )
                .unwrap_or_else(|| self.materialize_global_expression(argument));
            let object_candidate = if matches!(
                argument,
                Expression::Member { property, .. }
                    if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
            ) {
                None
            } else if matches!(
                materialized_argument,
                Expression::Member { ref property, .. }
                    if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
            ) {
                None
            } else if matches!(
                materialized_argument,
                Expression::Number(_)
                    | Expression::BigInt(_)
                    | Expression::String(_)
                    | Expression::Bool(_)
                    | Expression::Null
                    | Expression::Undefined
            ) {
                None
            } else {
                self.infer_global_object_binding(argument)
            };
            register_object_candidate(param_name, object_candidate);
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
                parameter_array_bindings.insert(param_name.to_string(), None);
                parameter_object_bindings.insert(param_name.to_string(), None);
            }
        }
    }
}
