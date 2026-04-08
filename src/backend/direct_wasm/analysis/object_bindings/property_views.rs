use super::*;

pub(in crate::backend::direct_wasm) fn enumerated_keys_from_object_binding(
    object_binding: &ObjectValueBinding,
) -> ArrayValueBinding {
    ArrayValueBinding {
        values: ordered_object_property_names(object_binding)
            .into_iter()
            .filter(|name| {
                !object_binding
                    .non_enumerable_string_properties
                    .iter()
                    .any(|hidden_name| hidden_name == name)
            })
            .map(|name| Some(Expression::String(name)))
            .collect(),
    }
}

pub(in crate::backend::direct_wasm) fn own_property_names_from_object_binding(
    object_binding: &ObjectValueBinding,
) -> ArrayValueBinding {
    ArrayValueBinding {
        values: ordered_object_property_names(object_binding)
            .into_iter()
            .map(|name| Some(Expression::String(name)))
            .collect(),
    }
}

pub(in crate::backend::direct_wasm) fn own_property_names_from_function_binding(
    object_binding: Option<&ObjectValueBinding>,
) -> ArrayValueBinding {
    let mut integer_keys = Vec::new();
    let mut other_keys = Vec::new();

    if let Some(object_binding) = object_binding {
        for name in ordered_object_property_names(object_binding) {
            if canonical_array_index_from_property_name(&name).is_some() {
                integer_keys.push(name);
            } else if name != "length" && name != "name" && name != "prototype" {
                other_keys.push(name);
            }
        }
    }

    let mut values = integer_keys
        .into_iter()
        .map(|name| Some(Expression::String(name)))
        .collect::<Vec<_>>();
    values.extend(
        ["length", "name", "prototype"]
            .into_iter()
            .map(|name| Some(Expression::String(name.to_string()))),
    );
    values.extend(
        other_keys
            .into_iter()
            .map(|name| Some(Expression::String(name))),
    );
    ArrayValueBinding { values }
}

pub(in crate::backend::direct_wasm) fn own_property_symbols_from_object_binding(
    object_binding: &ObjectValueBinding,
) -> ArrayValueBinding {
    ArrayValueBinding {
        values: object_binding
            .symbol_properties
            .iter()
            .map(|(key, _)| Some(key.clone()))
            .collect(),
    }
}

pub(in crate::backend::direct_wasm) fn own_property_names_from_array_binding(
    array_binding: &ArrayValueBinding,
) -> ArrayValueBinding {
    let mut values = array_binding
        .values
        .iter()
        .enumerate()
        .filter(|(_, value)| value.is_some())
        .map(|(index, _)| Some(Expression::String(index.to_string())))
        .collect::<Vec<_>>();
    values.push(Some(Expression::String("length".to_string())));
    ArrayValueBinding { values }
}

pub(in crate::backend::direct_wasm) fn object_binding_from_array_binding(
    array_binding: &ArrayValueBinding,
) -> ObjectValueBinding {
    let mut object_binding = empty_object_value_binding();
    for (index, value) in array_binding.values.iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        object_binding_set_property(
            &mut object_binding,
            Expression::String(index.to_string()),
            value.clone(),
        );
    }
    object_binding_set_property(
        &mut object_binding,
        Expression::String("length".to_string()),
        Expression::Number(array_binding.values.len() as f64),
    );
    object_binding_set_string_property_enumerable(&mut object_binding, "length", false);
    object_binding
}

pub(in crate::backend::direct_wasm) fn ordered_object_property_names(
    object_binding: &ObjectValueBinding,
) -> Vec<String> {
    let mut integer_keys = Vec::new();
    let mut other_keys = Vec::new();

    for (name, _) in &object_binding.string_properties {
        if let Some(index) = canonical_array_index_from_property_name(name) {
            integer_keys.push((index, name.clone()));
        } else {
            other_keys.push(name.clone());
        }
    }

    integer_keys.sort_by_key(|(index, _)| *index);
    integer_keys
        .into_iter()
        .map(|(_, name)| name)
        .chain(other_keys)
        .collect()
}

pub(in crate::backend::direct_wasm) fn is_internal_user_function_identifier(name: &str) -> bool {
    name.starts_with("__ayy_")
}

pub(in crate::backend::direct_wasm) fn object_binding_set_string_property_enumerable(
    object_binding: &mut ObjectValueBinding,
    property_name: &str,
    enumerable: bool,
) {
    if enumerable {
        object_binding
            .non_enumerable_string_properties
            .retain(|hidden_name| hidden_name != property_name);
    } else if !object_binding
        .non_enumerable_string_properties
        .iter()
        .any(|hidden_name| hidden_name == property_name)
    {
        object_binding
            .non_enumerable_string_properties
            .push(property_name.to_string());
    }
}
