use super::*;

pub(in crate::backend::direct_wasm) fn materialize_identifier_in_environment<
    Materializer: StaticIdentifierMaterializer + ?Sized,
    Environment: StaticIdentifierBindingEnvironment,
>(
    materializer: &Materializer,
    name: &str,
    environment: &Environment,
    recurse: &impl Fn(&Expression, &Environment) -> Option<Expression>,
) -> Option<Expression> {
    if materializer.preserves_symbol_identifier(name) {
        return Some(Expression::Identifier(name.to_string()));
    }
    if materializer.preserves_undefined_identifier(name) {
        return Some(Expression::Undefined);
    }
    if environment
        .global_value_binding(name)
        .is_some_and(|value| materializer.preserves_symbol_call_binding(value))
    {
        return Some(Expression::Identifier(name.to_string()));
    }
    if let Some(value) = environment.local_binding(name) {
        if environment.contains_object_binding(name)
            && materializer.preserves_object_identifier_binding(value, true)
        {
            return Some(Expression::Identifier(name.to_string()));
        }
        return recurse(value, environment);
    }
    if let Some(value) = environment.global_value_binding(name) {
        if environment.contains_object_binding(name)
            && materializer.preserves_object_identifier_binding(value, false)
        {
            return Some(Expression::Identifier(name.to_string()));
        }
        if !matches!(value, Expression::Identifier(alias) if alias == name) {
            return recurse(value, environment);
        }
    }
    Some(Expression::Identifier(name.to_string()))
}
