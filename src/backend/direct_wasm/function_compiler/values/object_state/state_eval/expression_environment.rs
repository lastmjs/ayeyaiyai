use super::super::*;
use super::FunctionStaticEvalContext;

impl StaticExpressionEnvironmentSource for FunctionStaticEvalContext<'_, '_> {
    type Environment = StaticResolutionEnvironment;
}

impl StaticBindingLookupSource for FunctionStaticEvalContext<'_, '_> {}

impl StaticMemberDeletionSource for FunctionStaticEvalContext<'_, '_> {}

impl StaticEnvironmentObjectBindingSource for FunctionStaticEvalContext<'_, '_> {
    fn static_resolve_environment_object_binding(
        &self,
        binding_expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<ObjectValueBinding> {
        self.resolve_object_binding_with_state(binding_expression, environment)
    }
}

impl StaticMissingMemberPolicySource for FunctionStaticEvalContext<'_, '_> {}
