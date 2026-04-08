use crate::backend::direct_wasm::{
    Expression, StaticBindingEnvironment, StaticLocalBindingEnvironment,
    StaticMutableObjectBindingEnvironment, StaticObjectBindingLookupEnvironment,
    assign_static_binding_with_object_sync, empty_object_value_binding,
    object_binding_remove_property, object_binding_set_property,
    resolve_stateful_object_binding_name_in_environment,
};

use super::StaticExpressionMaterialization;

pub(in crate::backend::direct_wasm) trait StaticBindingMutationExecutor:
    StaticExpressionMaterialization
{
    fn delete_member_property(
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

    fn initialize_binding_value(
        &self,
        name: &str,
        value: Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        let binding_expression = environment.set_local_binding(name.to_string(), value);
        let object_binding =
            self.resolve_environment_object_binding(&binding_expression, environment);
        environment.sync_object_binding(name, object_binding);
        Some(())
    }

    fn assign_binding_value(
        &self,
        name: &str,
        value: Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        assign_static_binding_with_object_sync(
            name,
            value,
            environment,
            |binding_expression, environment| {
                self.resolve_assigned_object_binding(binding_expression, environment)
            },
        );
        Some(())
    }

    fn assign_member_binding_value(
        &self,
        object: &Expression,
        property: Expression,
        value: Expression,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        let property = self.normalize_assigned_member_property(property, environment);
        let target_name = resolve_stateful_object_binding_name_in_environment(object, environment)?;
        self.prepare_assigned_member_target(&target_name, environment)?;
        let binding = environment.object_binding_mut(&target_name)?;
        object_binding_set_property(binding, property, value);
        Some(())
    }

    fn normalize_assigned_member_property(
        &self,
        property: Expression,
        environment: &mut Self::Environment,
    ) -> Expression {
        self.resolve_assigned_member_property_key(&property, environment)
            .unwrap_or(property)
    }

    fn resolve_assigned_member_property_key(
        &self,
        _property: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<Expression> {
        None
    }

    fn prepare_assigned_member_target(
        &self,
        target_name: &str,
        environment: &mut Self::Environment,
    ) -> Option<()> {
        if !environment.contains_object_binding(target_name)
            && self.should_seed_assigned_member_target_object_binding(target_name, environment)
        {
            environment.set_object_binding(target_name.to_string(), empty_object_value_binding());
        }
        Some(())
    }

    fn should_seed_assigned_member_target_object_binding(
        &self,
        _target_name: &str,
        _environment: &mut Self::Environment,
    ) -> bool {
        false
    }
}
