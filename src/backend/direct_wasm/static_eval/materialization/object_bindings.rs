use super::*;

pub(in crate::backend::direct_wasm) fn copy_enumerable_object_binding_properties(
    source_binding: &ObjectValueBinding,
    mut resolve_property_value: impl FnMut(&Expression) -> Option<Expression>,
) -> ObjectValueBinding {
    let mut copied_binding = empty_object_value_binding();
    for name in ordered_object_property_names(source_binding) {
        if source_binding
            .non_enumerable_string_properties
            .iter()
            .any(|hidden_name| hidden_name == &name)
        {
            continue;
        }
        let property = Expression::String(name.clone());
        let copied_value = resolve_property_value(&property).unwrap_or(Expression::Undefined);
        object_binding_set_property(&mut copied_binding, property, copied_value);
    }
    for (property, _) in &source_binding.symbol_properties {
        let copied_value = resolve_property_value(property).unwrap_or(Expression::Undefined);
        object_binding_set_property(&mut copied_binding, property.clone(), copied_value);
    }
    copied_binding
}

pub(in crate::backend::direct_wasm) fn resolve_copy_data_properties_binding<Context>(
    expression: &Expression,
    context: &mut Context,
    mut resolve_object_binding: impl FnMut(&Expression, &mut Context) -> Option<ObjectValueBinding>,
    mut resolve_member_getter_value: impl FnMut(
        &Expression,
        &Expression,
        &mut Context,
    ) -> Option<Expression>,
) -> Option<ObjectValueBinding> {
    let source_binding = resolve_object_binding(expression, context)?;
    Some(copy_enumerable_object_binding_properties(
        &source_binding,
        |property| {
            resolve_member_getter_value(expression, property, context).or_else(|| {
                resolve_object_binding(expression, context)
                    .and_then(|binding| object_binding_lookup_value(&binding, property).cloned())
            })
        },
    ))
}

pub(in crate::backend::direct_wasm) fn resolve_structural_object_binding<Context>(
    entries: &[ObjectEntry],
    context: &mut Context,
    mut materialize_expression: impl FnMut(&Expression, &mut Context) -> Option<Expression>,
    skip_spread_expression: impl Fn(&Expression, &Context) -> bool,
    mut resolve_copy_data_properties: impl FnMut(
        &Expression,
        &mut Context,
    ) -> Option<ObjectValueBinding>,
) -> Option<ObjectValueBinding> {
    let mut object_binding = empty_object_value_binding();
    for entry in entries {
        match entry {
            ObjectEntry::Data { key, value } => {
                let key = materialize_expression(key, context)?;
                let value = materialize_expression(value, context)?;
                object_binding_set_property(&mut object_binding, key, value);
            }
            ObjectEntry::Getter { key, .. } | ObjectEntry::Setter { key, .. } => {
                let key = materialize_expression(key, context)?;
                object_binding_set_property(&mut object_binding, key, Expression::Undefined);
            }
            ObjectEntry::Spread(expression) => {
                let spread_expression = materialize_expression(expression, context)?;
                if matches!(spread_expression, Expression::Null | Expression::Undefined)
                    || skip_spread_expression(&spread_expression, context)
                {
                    continue;
                }
                let spread_binding = resolve_copy_data_properties(&spread_expression, context)?;
                merge_enumerable_object_binding(&mut object_binding, &spread_binding);
            }
        }
    }
    Some(object_binding)
}

pub(in crate::backend::direct_wasm) fn resolve_structural_object_binding_in_environment<
    Executor,
    Environment,
    MaterializeExpression,
    ResolveObjectBinding,
    ResolveMemberGetterValue,
>(
    executor: &Executor,
    entries: &[ObjectEntry],
    environment: &mut Environment,
    materialize_expression: &MaterializeExpression,
    resolve_object_binding: &ResolveObjectBinding,
    resolve_member_getter_value: &ResolveMemberGetterValue,
) -> Option<ObjectValueBinding>
where
    Executor: StaticIdentifierMaterializer + ?Sized,
    MaterializeExpression: Fn(&Expression, &mut Environment) -> Option<Expression>,
    ResolveObjectBinding: Fn(&Expression, &mut Environment) -> Option<ObjectValueBinding>,
    ResolveMemberGetterValue: Fn(&Expression, &Expression, &mut Environment) -> Option<Expression>,
{
    resolve_structural_object_binding(
        entries,
        environment,
        |expression, environment| materialize_expression(expression, environment),
        |spread_expression, _| {
            matches!(
                spread_expression,
                Expression::Identifier(name)
                    if name == "undefined"
                        && executor.is_unshadowed_builtin_identifier(name)
            )
        },
        |spread_expression, environment| {
            resolve_copy_data_properties_binding(
                spread_expression,
                environment,
                |expression, environment| resolve_object_binding(expression, environment),
                |object, property, environment| {
                    resolve_member_getter_value(object, property, environment)
                },
            )
        },
    )
}

pub(in crate::backend::direct_wasm) fn resolve_specialized_object_binding_expression<Context>(
    expression: &Expression,
    context: &mut Context,
    mut resolve_array_binding: impl FnMut(&Expression, &mut Context) -> Option<ArrayValueBinding>,
    mut resolve_object_entries: impl FnMut(&[ObjectEntry], &mut Context) -> Option<ObjectValueBinding>,
    is_object_create_call: impl Fn(&Expression, &mut Context) -> bool,
    mut resolve_fallback: impl FnMut(&Expression, &mut Context) -> Option<ObjectValueBinding>,
) -> Option<ObjectValueBinding> {
    match expression {
        Expression::Array(_) => resolve_array_binding(expression, context)
            .map(|binding| object_binding_from_array_binding(&binding)),
        Expression::Object(entries) => resolve_object_entries(entries, context),
        Expression::Call { .. } if is_object_create_call(expression, context) => {
            Some(empty_object_value_binding())
        }
        _ => resolve_fallback(expression, context),
    }
}
