use crate::backend::direct_wasm::{
    Expression, ObjectValueBinding, StaticExpressionExecutionSource,
};

use super::contract::{
    StaticBindingMutationExecutor, StaticExecutorContext, StaticExpressionHooks,
    StaticExpressionMaterialization,
};

impl<T> StaticExecutorContext for T
where
    T: StaticExpressionExecutionSource + ?Sized,
{
    type Environment = T::Environment;
}

impl<T> StaticExpressionHooks for T
where
    T: StaticExpressionExecutionSource + ?Sized,
{
    fn lookup_binding_value(
        &self,
        name: &str,
        environment: &Self::Environment,
    ) -> Option<Expression> {
        self.static_lookup_binding_value(name, environment)
    }

    fn evaluate_special_expression(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        self.static_evaluate_special_expression(expression, environment)
    }
}

impl<T> StaticExpressionMaterialization for T
where
    T: StaticExpressionExecutionSource + ?Sized,
{
    fn preserve_missing_member_expression(
        &self,
        full_expression: &Expression,
        object: &Expression,
        property: &Expression,
        environment: &Self::Environment,
    ) -> bool {
        self.static_preserve_missing_member_expression(
            full_expression,
            object,
            property,
            environment,
        )
    }

    fn preserve_new_expressions_in_materialization(&self) -> bool {
        self.static_preserve_new_expressions_in_materialization()
    }

    fn preserve_call_expressions_in_materialization(&self) -> bool {
        self.static_preserve_call_expressions_in_materialization()
    }

    fn materialize_post_structural_fallback_expression(
        &self,
        expression: &Expression,
        environment: &Self::Environment,
    ) -> Option<Expression> {
        self.static_materialize_post_structural_fallback_expression(expression, environment)
    }

    fn resolve_environment_object_binding(
        &self,
        binding_expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<ObjectValueBinding> {
        self.static_resolve_environment_object_binding(binding_expression, environment)
    }
}

impl<T> StaticBindingMutationExecutor for T
where
    T: StaticExpressionExecutionSource + ?Sized,
{
    fn resolve_assigned_member_property_key(
        &self,
        property: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        self.static_resolve_assigned_member_property_key(property, environment)
    }

    fn should_seed_assigned_member_target_object_binding(
        &self,
        target_name: &str,
        environment: &mut Self::Environment,
    ) -> bool {
        self.static_should_seed_assigned_member_target_object_binding(target_name, environment)
    }
}
