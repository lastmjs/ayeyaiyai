use super::super::*;
use super::ProgramStaticEvalContext;

impl StaticUserFunctionBindingSource for ProgramStaticEvalContext<'_> {
    fn static_user_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.registered_function(function_name)
    }

    fn static_user_function_metadata(&self, function_name: &str) -> Option<&UserFunction> {
        self.user_function(function_name)
    }

    fn substitute_static_user_function_arguments(
        &self,
        expression: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> Expression {
        self.substitute_user_function_arguments(expression, user_function, arguments)
    }
}
