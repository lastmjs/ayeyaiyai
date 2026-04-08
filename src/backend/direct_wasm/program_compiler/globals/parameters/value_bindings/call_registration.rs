use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn register_parameter_value_bindings_for_call(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
        bindings: &mut HashMap<String, HashMap<String, Option<Expression>>>,
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

        for (index, argument) in call_arguments.iter().enumerate() {
            if index >= user_function.params.len() {
                break;
            }
            let param_name = &user_function.params[index];
            let materialized_argument =
                self.with_cloned_global_binding_state(|value_bindings, object_bindings| {
                    self.materialize_global_expression_with_state(
                        argument,
                        &HashMap::new(),
                        value_bindings,
                        object_bindings,
                    )
                    .unwrap_or_else(|| self.materialize_global_expression(argument))
                });
            let effective_argument = if matches!(
                argument,
                Expression::Member { property, .. }
                    if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
            ) {
                argument.clone()
            } else if matches!(
                materialized_argument,
                Expression::Member { ref property, .. }
                    if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
            ) {
                materialized_argument
            } else if matches!(
                materialized_argument,
                Expression::Number(_)
                    | Expression::BigInt(_)
                    | Expression::String(_)
                    | Expression::Bool(_)
                    | Expression::Null
                    | Expression::Undefined
            ) {
                materialized_argument
            } else {
                self.infer_global_object_binding(argument)
                    .map(|binding| object_binding_to_expression(&binding))
                    .unwrap_or(materialized_argument)
            };
            match parameter_bindings.get(param_name) {
                Some(None) => {}
                Some(Some(existing)) if *existing == effective_argument => {}
                Some(Some(_)) => {
                    parameter_bindings.insert(param_name.to_string(), None);
                }
                None => {
                    parameter_bindings.insert(param_name.to_string(), Some(effective_argument));
                }
            }
        }

        if call_arguments.len() < user_function.params.len() {
            for param_name in user_function.params.iter().skip(call_arguments.len()) {
                parameter_bindings.insert(param_name.to_string(), None);
            }
        }
    }
}
