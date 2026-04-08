use super::super::*;
use super::ProgramStaticEvalContext;

impl StaticExpressionEnvironmentSource for ProgramStaticEvalContext<'_> {
    type Environment = GlobalStaticEvaluationEnvironment;
}

impl StaticBindingLookupSource for ProgramStaticEvalContext<'_> {}

impl StaticMemberDeletionSource for ProgramStaticEvalContext<'_> {}

impl StaticSpecialExpressionSource for ProgramStaticEvalContext<'_> {}

impl StaticEnvironmentObjectBindingSource for ProgramStaticEvalContext<'_> {
    fn static_resolve_environment_object_binding(
        &self,
        binding_expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<ObjectValueBinding> {
        self.infer_object_binding_with_state(binding_expression, environment)
    }
}

impl StaticMissingMemberPolicySource for ProgramStaticEvalContext<'_> {
    fn static_preserve_missing_member_expression(
        &self,
        _full_expression: &Expression,
        object: &Expression,
        property: &Expression,
        _environment: &Self::Environment,
    ) -> bool {
        self.preserves_missing_member_function_capture(object, property)
    }
}

impl StaticMaterializationPolicySource for ProgramStaticEvalContext<'_> {}

impl StaticAssignedMemberPolicySource for ProgramStaticEvalContext<'_> {}
