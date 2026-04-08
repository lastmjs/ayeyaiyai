use super::*;

pub(in crate::backend::direct_wasm) fn assign_static_binding_with_object_sync<
    Environment: StaticBindingEnvironment,
>(
    name: &str,
    value: Expression,
    environment: &mut Environment,
    resolve_object_binding: impl FnOnce(&Expression, &mut Environment) -> Option<ObjectValueBinding>,
) -> Expression {
    let binding_expression = environment.assign_binding_value(name.to_string(), value.clone());
    let object_binding = resolve_object_binding(&binding_expression, environment);
    environment.sync_object_binding(name, object_binding);
    value
}
