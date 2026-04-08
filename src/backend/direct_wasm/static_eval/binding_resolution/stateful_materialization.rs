use super::*;

pub(in crate::backend::direct_wasm) fn materialize_stateful_expression_in_environment<
    Materializer,
    Environment,
    ResolveObjectBinding,
    MaterializeMemberMiss,
    MaterializeFallback,
>(
    materializer: &Materializer,
    expression: &Expression,
    environment: &Environment,
    resolve_object_binding: &ResolveObjectBinding,
    materialize_member_miss: &MaterializeMemberMiss,
    materialize_fallback: &MaterializeFallback,
) -> Option<Expression>
where
    Materializer: StaticIdentifierMaterializer + ?Sized,
    Environment: StaticIdentifierBindingEnvironment,
    ResolveObjectBinding: Fn(&Expression, &Environment) -> Option<ObjectValueBinding>,
    MaterializeMemberMiss: Fn(
        &Expression,
        &Expression,
        Expression,
        &Environment,
        &dyn Fn(&Expression, &Environment) -> Option<Expression>,
    ) -> Option<Expression>,
    MaterializeFallback: Fn(
        &Expression,
        &Environment,
        &dyn Fn(&Expression, &Environment) -> Option<Expression>,
    ) -> Option<Expression>,
{
    fn recurse<
        Materializer,
        Environment,
        ResolveObjectBinding,
        MaterializeMemberMiss,
        MaterializeFallback,
    >(
        materializer: &Materializer,
        expression: &Expression,
        environment: &Environment,
        resolve_object_binding: &ResolveObjectBinding,
        materialize_member_miss: &MaterializeMemberMiss,
        materialize_fallback: &MaterializeFallback,
    ) -> Option<Expression>
    where
        Materializer: StaticIdentifierMaterializer + ?Sized,
        Environment: StaticIdentifierBindingEnvironment,
        ResolveObjectBinding: Fn(&Expression, &Environment) -> Option<ObjectValueBinding>,
        MaterializeMemberMiss: Fn(
            &Expression,
            &Expression,
            Expression,
            &Environment,
            &dyn Fn(&Expression, &Environment) -> Option<Expression>,
        ) -> Option<Expression>,
        MaterializeFallback: Fn(
            &Expression,
            &Environment,
            &dyn Fn(&Expression, &Environment) -> Option<Expression>,
        ) -> Option<Expression>,
    {
        let recurse_fn = |expression: &Expression, environment: &Environment| {
            recurse(
                materializer,
                expression,
                environment,
                resolve_object_binding,
                materialize_member_miss,
                materialize_fallback,
            )
        };
        match expression {
            Expression::Identifier(name) => {
                materialize_identifier_in_environment(materializer, name, environment, &recurse_fn)
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::This
            | Expression::NewTarget
            | Expression::Sent => Some(expression.clone()),
            Expression::Member { object, property } => {
                let resolved_property = recurse_fn(property, environment)?;
                if let Some(value) = materialize_member_from_object_binding(
                    resolve_object_binding(object, environment),
                    &resolved_property,
                    |value| recurse_fn(value, environment),
                ) {
                    return Some(value);
                }
                materialize_member_miss(
                    expression,
                    object,
                    resolved_property,
                    environment,
                    &recurse_fn,
                )
            }
            _ => materialize_fallback(expression, environment, &recurse_fn),
        }
    }

    recurse(
        materializer,
        expression,
        environment,
        resolve_object_binding,
        materialize_member_miss,
        materialize_fallback,
    )
}
