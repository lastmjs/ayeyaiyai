use super::*;

pub(in crate::backend::direct_wasm) fn materialize_object_member_from_binding(
    object_binding: &ObjectValueBinding,
    property: &Expression,
    recurse: impl FnOnce(&Expression) -> Option<Expression>,
) -> Option<Expression> {
    if let Some(value) = object_binding_lookup_value(object_binding, property) {
        return recurse(value);
    }
    if static_property_name_from_expression(property).is_some()
        || object_binding_has_property(object_binding, property)
    {
        return Some(Expression::Undefined);
    }
    None
}

pub(in crate::backend::direct_wasm) fn materialize_member_from_object_binding(
    object_binding: Option<ObjectValueBinding>,
    property: &Expression,
    recurse: impl FnOnce(&Expression) -> Option<Expression>,
) -> Option<Expression> {
    let object_binding = object_binding?;
    materialize_object_member_from_binding(&object_binding, property, recurse)
}

pub(in crate::backend::direct_wasm) fn materialize_missing_member_expression_with_policy<
    Environment,
>(
    full_expression: &Expression,
    object: &Expression,
    property: Expression,
    environment: &Environment,
    recurse: &dyn Fn(&Expression, &Environment) -> Option<Expression>,
    preserve_missing: &dyn Fn(&Expression, &Expression, &Expression, &Environment) -> bool,
) -> Option<Expression> {
    if preserve_missing(full_expression, object, &property, environment) {
        return Some(full_expression.clone());
    }
    let materialized = Expression::Member {
        object: Box::new(recurse(object, environment)?),
        property: Box::new(property),
    };
    if static_expression_matches(&materialized, full_expression) {
        None
    } else {
        recurse(&materialized, environment)
    }
}

pub(in crate::backend::direct_wasm) fn preserves_missing_member_function_capture<Key>(
    object: &Expression,
    property: &Expression,
    resolve_key: impl Fn(&Expression, &Expression) -> Option<Key>,
    has_capture_slots: impl Fn(&Key) -> bool,
) -> bool {
    resolve_key(object, property).is_some_and(|key| has_capture_slots(&key))
}
