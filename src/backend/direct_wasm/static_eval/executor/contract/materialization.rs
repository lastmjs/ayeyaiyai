use crate::backend::direct_wasm::{
    Expression, ObjectValueBinding, StaticTransactionalEnvironment,
    materialize_missing_member_expression_with_policy,
    materialize_stateful_expression_in_environment, materialize_structural_expression,
};

use super::StaticExecutorContext;

pub(in crate::backend::direct_wasm) trait StaticExpressionMaterialization:
    StaticExecutorContext
{
    fn materialize_expression(
        &self,
        expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<Expression> {
        materialize_stateful_expression_in_environment(
            self,
            expression,
            environment,
            &|expression, environment| {
                self.resolve_materialized_object_binding(expression, environment)
            },
            &|full_expression, object, property, environment, recurse| {
                self.materialize_missing_member_expression(
                    full_expression,
                    object,
                    property,
                    environment,
                    recurse,
                )
            },
            &|expression, environment, recurse| {
                self.materialize_fallback_expression(expression, environment, recurse)
            },
        )
    }

    fn materialize_expression_in_forked_environment(
        &self,
        expression: &Expression,
        environment: &Self::Environment,
    ) -> Option<Expression> {
        let mut environment = environment.fork_environment();
        self.materialize_expression(expression, &mut environment)
    }

    fn resolve_materialized_object_binding(
        &self,
        expression: &Expression,
        environment: &Self::Environment,
    ) -> Option<ObjectValueBinding> {
        let mut environment = environment.fork_environment();
        self.resolve_assigned_object_binding(expression, &mut environment)
    }

    fn materialize_missing_member_expression(
        &self,
        full_expression: &Expression,
        object: &Expression,
        property: Expression,
        environment: &Self::Environment,
        recurse: &dyn Fn(&Expression, &Self::Environment) -> Option<Expression>,
    ) -> Option<Expression> {
        materialize_missing_member_expression_with_policy(
            full_expression,
            object,
            property,
            environment,
            recurse,
            &|full_expression, object, property, environment| {
                self.preserve_missing_member_expression(
                    full_expression,
                    object,
                    property,
                    environment,
                )
            },
        )
    }

    fn preserve_missing_member_expression(
        &self,
        _full_expression: &Expression,
        _object: &Expression,
        _property: &Expression,
        _environment: &Self::Environment,
    ) -> bool {
        false
    }

    fn preserve_new_expressions_in_materialization(&self) -> bool {
        false
    }

    fn preserve_call_expressions_in_materialization(&self) -> bool {
        false
    }

    fn materialize_post_structural_fallback_expression(
        &self,
        _expression: &Expression,
        _environment: &Self::Environment,
    ) -> Option<Expression> {
        None
    }

    fn materialize_fallback_expression(
        &self,
        expression: &Expression,
        environment: &Self::Environment,
        recurse: &dyn Fn(&Expression, &Self::Environment) -> Option<Expression>,
    ) -> Option<Expression> {
        materialize_structural_expression(
            expression,
            self.preserve_new_expressions_in_materialization(),
            self.preserve_call_expressions_in_materialization(),
            environment,
            recurse,
        )
        .or_else(|| self.materialize_post_structural_fallback_expression(expression, environment))
    }

    fn resolve_assigned_object_binding(
        &self,
        binding_expression: &Expression,
        environment: &mut Self::Environment,
    ) -> Option<ObjectValueBinding> {
        self.resolve_environment_object_binding(binding_expression, environment)
    }

    fn resolve_environment_object_binding(
        &self,
        _binding_expression: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<ObjectValueBinding> {
        None
    }
}
