use super::*;

pub(in crate::backend::direct_wasm) trait StaticExpressionEnvironmentSource:
    StaticIdentifierMaterializer
{
    type Environment: StaticBindingEnvironment
        + StaticLocalBindingEnvironment
        + StaticMutableObjectBindingEnvironment
        + StaticIdentifierBindingEnvironment
        + StaticTransactionalEnvironment;
}

pub(in crate::backend::direct_wasm) trait StaticBindingLookupSource:
    StaticExpressionEnvironmentSource
{
    fn static_lookup_binding_value(
        &self,
        name: &str,
        environment: &Self::Environment,
    ) -> Option<Expression> {
        environment.binding(name).cloned()
    }
}

pub(in crate::backend::direct_wasm) trait StaticMemberDeletionSource:
    StaticExpressionEnvironmentSource
{
    fn static_delete_member_property(
        &self,
        object: &Expression,
        property: Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        let target_name = resolve_stateful_object_binding_name_in_environment(object, environment)?;
        let binding = environment.object_binding_mut(&target_name)?;
        object_binding_remove_property(binding, &property);
        Some(())
    }
}

pub(in crate::backend::direct_wasm) trait StaticSpecialExpressionSource:
    StaticExpressionEnvironmentSource
{
    fn static_evaluate_special_expression(
        &self,
        _expression: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<Expression> {
        None
    }
}

pub(in crate::backend::direct_wasm) trait StaticMissingMemberPolicySource:
    StaticExpressionEnvironmentSource
{
    fn static_preserve_missing_member_expression(
        &self,
        _full_expression: &Expression,
        _object: &Expression,
        _property: &Expression,
        _environment: &Self::Environment,
    ) -> bool {
        false
    }
}

pub(in crate::backend::direct_wasm) trait StaticMaterializationPolicySource:
    StaticExpressionEnvironmentSource
{
    fn static_preserve_new_expressions_in_materialization(&self) -> bool {
        false
    }

    fn static_preserve_call_expressions_in_materialization(&self) -> bool {
        false
    }

    fn static_materialize_post_structural_fallback_expression(
        &self,
        _expression: &Expression,
        _environment: &Self::Environment,
    ) -> Option<Expression> {
        None
    }
}

pub(in crate::backend::direct_wasm) trait StaticEnvironmentObjectBindingSource:
    StaticExpressionEnvironmentSource
{
    fn static_resolve_environment_object_binding(
        &self,
        _binding_expression: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<ObjectValueBinding> {
        None
    }
}

pub(in crate::backend::direct_wasm) trait StaticAssignedMemberPolicySource:
    StaticExpressionEnvironmentSource
{
    fn static_resolve_assigned_member_property_key(
        &self,
        _property: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<Expression> {
        None
    }

    fn static_should_seed_assigned_member_target_object_binding(
        &self,
        _target_name: &str,
        _environment: &mut Self::Environment,
    ) -> bool {
        false
    }
}

pub(in crate::backend::direct_wasm) trait StaticExpressionExecutionSource:
    StaticExpressionEnvironmentSource
    + StaticBindingLookupSource
    + StaticMemberDeletionSource
    + StaticSpecialExpressionSource
    + StaticMissingMemberPolicySource
    + StaticMaterializationPolicySource
    + StaticEnvironmentObjectBindingSource
    + StaticAssignedMemberPolicySource
{
}

impl<T> StaticExpressionExecutionSource for T where
    T: StaticExpressionEnvironmentSource
        + StaticBindingLookupSource
        + StaticMemberDeletionSource
        + StaticSpecialExpressionSource
        + StaticMissingMemberPolicySource
        + StaticMaterializationPolicySource
        + StaticEnvironmentObjectBindingSource
        + StaticAssignedMemberPolicySource
{
}
