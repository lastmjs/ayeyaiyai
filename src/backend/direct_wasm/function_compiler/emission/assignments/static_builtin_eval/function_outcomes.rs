use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_function_outcome_from_binding_with_context(
        &self,
        binding: &LocalFunctionBinding,
        arguments: &[CallArgument],
        current_function_name: Option<&str>,
    ) -> Option<StaticEvalOutcome> {
        let LocalFunctionBinding::User(function_name) = binding else {
            let LocalFunctionBinding::Builtin(function_name) = binding else {
                return None;
            };
            return self.resolve_static_builtin_function_outcome(
                function_name,
                arguments,
                current_function_name,
            );
        };
        let user_function = self.user_function(function_name)?;

        let function = self.resolve_registered_function_declaration(function_name)?;
        if function.body.is_empty() {
            return Some(StaticEvalOutcome::Value(Expression::Undefined));
        }
        let [statement] = function.body.as_slice() else {
            return None;
        };
        match statement {
            Statement::Return(expression) => Some(StaticEvalOutcome::Value(
                self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    arguments,
                ),
            )),
            Statement::Throw(expression) => Some(StaticEvalOutcome::Throw(
                StaticThrowValue::Value(self.substitute_user_function_argument_bindings(
                    expression,
                    user_function,
                    arguments,
                )),
            )),
            _ => None,
        }
    }
}
