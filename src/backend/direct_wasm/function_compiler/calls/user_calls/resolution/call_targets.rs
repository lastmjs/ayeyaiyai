use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_user_function_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<&UserFunction> {
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(expression)?
        else {
            return None;
        };
        self.backend
            .function_registry
            .catalog
            .user_function(&function_name)
    }

    pub(in crate::backend::direct_wasm) fn resolve_user_function_call_target(
        &self,
        expression: &Expression,
    ) -> Option<(UserFunction, Vec<CallArgument>)> {
        let (callee, arguments) = match expression {
            Expression::Call { callee, arguments } | Expression::New { callee, arguments } => {
                (callee.as_ref(), arguments.as_slice())
            }
            _ => return None,
        };

        if let Some(LocalFunctionBinding::User(function_name)) =
            self.resolve_function_binding_from_expression(callee)
        {
            let user_function = self
                .backend
                .function_registry
                .catalog
                .user_function(&function_name)?
                .clone();
            return Some((
                user_function,
                self.expand_call_arguments(arguments)
                    .into_iter()
                    .map(CallArgument::Expression)
                    .collect(),
            ));
        }

        let Expression::Call { .. } = expression else {
            return None;
        };
        let Expression::Member { object, property } = callee else {
            return None;
        };
        let Expression::String(property_name) = property.as_ref() else {
            return None;
        };
        if property_name != "call" && property_name != "apply" {
            return None;
        }

        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(object)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?.clone();
        let expanded_arguments = self.expand_call_arguments(arguments);
        let effective_arguments = if property_name == "call" {
            expanded_arguments
                .into_iter()
                .skip(1)
                .map(CallArgument::Expression)
                .collect()
        } else {
            let apply_expression = expanded_arguments
                .get(1)
                .cloned()
                .unwrap_or(Expression::Undefined);
            self.expand_apply_call_arguments_from_expression(&apply_expression)?
        };
        Some((user_function, effective_arguments))
    }
}
