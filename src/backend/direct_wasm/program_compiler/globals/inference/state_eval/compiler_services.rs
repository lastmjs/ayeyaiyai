use super::super::super::*;
use super::ProgramStaticEvalContext;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn static_eval_context(
        &self,
    ) -> ProgramStaticEvalContext<'_> {
        ProgramStaticEvalContext::new(self)
    }

    pub(in crate::backend::direct_wasm) fn evaluate_static_expression_with_state(
        &self,
        expression: &Expression,
        environment: &mut GlobalStaticEvaluationEnvironment,
    ) -> Option<Expression> {
        let context = self.static_eval_context();
        context.evaluate_static_expression_with_state(expression, environment)
    }

    pub(in crate::backend::direct_wasm) fn evaluate_static_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        let mut environment = GlobalStaticEvaluationEnvironment::from_snapshots(
            HashMap::new(),
            HashMap::new(),
            HashMap::new(),
        );
        self.evaluate_static_expression_with_state(expression, &mut environment)
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
