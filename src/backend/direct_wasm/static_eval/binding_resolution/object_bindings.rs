use super::*;

pub(in crate::backend::direct_wasm) fn resolve_stateful_object_binding_name_in_environment<
    Environment: StaticObjectBindingLookupEnvironment,
>(
    expression: &Expression,
    environment: &Environment,
) -> Option<String> {
    match expression {
        Expression::Identifier(name) if environment.contains_object_binding(name) => {
            Some(name.clone())
        }
        Expression::Identifier(name) => environment
            .binding(name)
            .filter(|value| !matches!(value, Expression::Identifier(alias) if alias == name))
            .and_then(|value| {
                resolve_stateful_object_binding_name_in_environment(value, environment)
            }),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn resolve_stateful_object_binding_from_environment<
    Environment: StaticObjectBindingEnvironment,
>(
    expression: &Expression,
    environment: &mut Environment,
    resolve_non_identifier: &impl Fn(&Expression, &mut Environment) -> Option<ObjectValueBinding>,
) -> Option<ObjectValueBinding> {
    match expression {
        Expression::Identifier(name) => environment.object_binding(name).cloned().or_else(|| {
            environment
                .binding(name)
                .filter(|value| !matches!(value, Expression::Identifier(alias) if alias == name))
                .cloned()
                .and_then(|value| {
                    resolve_stateful_object_binding_from_environment(
                        &value,
                        environment,
                        resolve_non_identifier,
                    )
                })
        }),
        _ => resolve_non_identifier(expression, environment),
    }
}
