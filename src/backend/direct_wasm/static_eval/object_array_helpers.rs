use super::*;

pub(in crate::backend::direct_wasm) fn infer_enumerated_keys_binding_from_expression(
    expression: &Expression,
    resolve_array_binding: impl Fn(&Expression) -> Option<ArrayValueBinding>,
    resolve_object_binding: impl Fn(&Expression) -> Option<ObjectValueBinding>,
) -> Option<ArrayValueBinding> {
    if let Some(array_binding) = resolve_array_binding(expression) {
        return Some(enumerated_keys_from_array_binding(&array_binding));
    }
    if let Some(object_binding) = resolve_object_binding(expression) {
        return Some(enumerated_keys_from_object_binding(&object_binding));
    }
    None
}

pub(in crate::backend::direct_wasm) fn infer_own_property_names_binding_from_expression(
    expression: &Expression,
    resolve_array_binding: impl Fn(&Expression) -> Option<ArrayValueBinding>,
    resolve_object_binding: impl Fn(&Expression) -> Option<ObjectValueBinding>,
    has_function_property_shape: impl Fn(&Expression) -> bool,
) -> Option<ArrayValueBinding> {
    if let Some(array_binding) = resolve_array_binding(expression) {
        return Some(own_property_names_from_array_binding(&array_binding));
    }
    let object_binding = resolve_object_binding(expression);
    if has_function_property_shape(expression) {
        return Some(own_property_names_from_function_binding(
            object_binding.as_ref(),
        ));
    }
    if let Some(object_binding) = object_binding {
        return Some(own_property_names_from_object_binding(&object_binding));
    }
    None
}

pub(in crate::backend::direct_wasm) fn infer_own_property_symbols_binding_from_expression(
    expression: &Expression,
    resolve_object_binding: impl Fn(&Expression) -> Option<ObjectValueBinding>,
) -> Option<ArrayValueBinding> {
    let object_binding = resolve_object_binding(expression)?;
    Some(own_property_symbols_from_object_binding(&object_binding))
}

pub(in crate::backend::direct_wasm) fn infer_builtin_object_array_call_binding(
    callee: &Expression,
    arguments: &[CallArgument],
    infer_enumerated_keys_binding: impl Fn(&Expression) -> Option<ArrayValueBinding>,
    infer_own_property_names_binding: impl Fn(&Expression) -> Option<ArrayValueBinding>,
    infer_own_property_symbols_binding: impl Fn(&Expression) -> Option<ArrayValueBinding>,
) -> Option<ArrayValueBinding> {
    let Expression::Member { object, property } = callee else {
        return None;
    };
    if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
        return None;
    }
    let [CallArgument::Expression(target), ..] = arguments else {
        return None;
    };
    match property.as_ref() {
        Expression::String(name) if name == "keys" => infer_enumerated_keys_binding(target),
        Expression::String(name) if name == "getOwnPropertyNames" => {
            infer_own_property_names_binding(target)
        }
        Expression::String(name) if name == "getOwnPropertySymbols" => {
            infer_own_property_symbols_binding(target)
        }
        _ => None,
    }
}
