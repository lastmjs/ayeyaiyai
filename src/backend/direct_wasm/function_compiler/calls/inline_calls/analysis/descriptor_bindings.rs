use super::*;

#[path = "descriptor_bindings/argument_scan.rs"]
mod argument_scan;
#[path = "descriptor_bindings/base_scan.rs"]
mod base_scan;
#[path = "descriptor_bindings/call_frame_scan.rs"]
mod call_frame_scan;

impl<'a> FunctionCompiler<'a> {
    fn descriptor_binding_call_arguments(arguments: &[Expression]) -> Vec<CallArgument> {
        arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect()
    }

    fn descriptor_binding_arguments_expression(arguments: &[Expression]) -> Expression {
        Expression::Array(
            arguments
                .iter()
                .cloned()
                .map(ArrayElement::Expression)
                .collect(),
        )
    }

    pub(in crate::backend::direct_wasm) fn user_function_creates_descriptor_binding_with_arguments(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> bool {
        self.resolve_registered_function_declaration(&user_function.name)
            .is_some_and(|function| {
                function.body.iter().any(|statement| {
                    self.statement_creates_descriptor_binding_with_arguments(
                        statement,
                        user_function,
                        arguments,
                    )
                })
            })
    }

    pub(in crate::backend::direct_wasm) fn user_function_creates_descriptor_binding_with_explicit_call_frame(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
        this_expression: &Expression,
    ) -> bool {
        self.resolve_registered_function_declaration(&user_function.name)
            .is_some_and(|function| {
                function.body.iter().any(|statement| {
                    self.statement_creates_descriptor_binding_with_explicit_call_frame(
                        statement,
                        user_function,
                        arguments,
                        this_expression,
                    )
                })
            })
    }
}
