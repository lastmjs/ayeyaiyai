use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_expression_with_aliases(
        &self,
        expression: &Expression,
        aliases: &HashMap<String, Option<LocalFunctionBinding>>,
    ) -> Option<LocalFunctionBinding> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(function_binding) = aliases.get(name) {
                    return function_binding.clone();
                }
                if is_internal_user_function_identifier(name) && self.contains_user_function(name) {
                    Some(LocalFunctionBinding::User(name.clone()))
                } else if let Some(function_binding) = self.global_function_binding(name) {
                    Some(function_binding.clone())
                } else if name == "eval" || infer_call_result_kind(name).is_some() {
                    Some(LocalFunctionBinding::Builtin(name.clone()))
                } else {
                    None
                }
            }
            Expression::Member { object, property } => {
                if let Some(key) = self.global_member_function_binding_key(object, property)
                    && let Some(binding) = self.global_member_function_binding(&key)
                {
                    return Some(binding.clone());
                }

                let materialized = self.materialize_global_expression(expression);
                if !static_expression_matches(&materialized, expression) {
                    return self.resolve_function_binding_from_expression_with_aliases(
                        &materialized,
                        aliases,
                    );
                }

                self.infer_global_function_binding(expression)
            }
            Expression::Sequence(expressions) => expressions.last().and_then(|expression| {
                self.resolve_function_binding_from_expression_with_aliases(expression, aliases)
            }),
            _ => {
                let materialized = self.materialize_global_expression(expression);
                if !static_expression_matches(&materialized, expression) {
                    return self.resolve_function_binding_from_expression_with_aliases(
                        &materialized,
                        aliases,
                    );
                }
                self.infer_global_function_binding(expression)
            }
        }
    }
}
