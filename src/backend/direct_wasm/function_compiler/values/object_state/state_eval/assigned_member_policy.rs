use super::super::*;
use super::FunctionStaticEvalContext;

impl StaticAssignedMemberPolicySource for FunctionStaticEvalContext<'_, '_> {
    fn static_resolve_assigned_member_property_key(
        &self,
        property: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<Expression> {
        self.resolve_property_key(property)
    }

    fn static_should_seed_assigned_member_target_object_binding(
        &self,
        target_name: &str,
        _environment: &mut Self::Environment,
    ) -> bool {
        self.resolve_function_binding(&Expression::Identifier(target_name.to_string()))
            .is_some()
    }
}
