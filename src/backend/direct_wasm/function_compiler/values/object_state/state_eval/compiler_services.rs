use super::super::super::*;
use super::FunctionStaticEvalContext;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn static_eval_context(
        &self,
    ) -> FunctionStaticEvalContext<'_, 'a> {
        FunctionStaticEvalContext::new(self)
    }

    pub(in crate::backend::direct_wasm) fn evaluate_static_expression_with_state(
        &self,
        expression: &Expression,
        environment: &mut StaticResolutionEnvironment,
    ) -> Option<Expression> {
        let context = self.static_eval_context();
        context.evaluate_static_expression_with_state(expression, environment)
    }

    pub(in crate::backend::direct_wasm) fn materialize_static_expression_with_state(
        &self,
        expression: &Expression,
        environment: &StaticResolutionEnvironment,
    ) -> Option<Expression> {
        let context = self.static_eval_context();
        context.materialize_static_expression_with_state(expression, environment)
    }

    pub(in crate::backend::direct_wasm) fn execute_static_statements_with_state(
        &self,
        statements: &[Statement],
        environment: &mut StaticResolutionEnvironment,
    ) -> Option<Option<Expression>> {
        let context = self.static_eval_context();
        context.execute_static_statements_with_state(statements, environment)
    }

    pub(in crate::backend::direct_wasm) fn static_enumerated_keys_binding(
        &self,
        expression: &Expression,
    ) -> Option<ArrayValueBinding> {
        let context = self.static_eval_context();
        StaticBuiltinArrayBindingResolver::static_enumerated_keys_binding(&context, expression)
    }

    pub(in crate::backend::direct_wasm) fn static_builtin_object_array_call_binding(
        &self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> Option<ArrayValueBinding> {
        let context = self.static_eval_context();
        StaticBuiltinArrayBindingResolver::static_builtin_object_array_call_binding(
            &context, callee, arguments,
        )
    }
}
