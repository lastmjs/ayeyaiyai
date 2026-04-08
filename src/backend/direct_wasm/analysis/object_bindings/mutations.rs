use super::*;

pub(in crate::backend::direct_wasm) fn object_binding_lookup_value<'a>(
    object_binding: &'a ObjectValueBinding,
    property: &Expression,
) -> Option<&'a Expression> {
    if let Some(property_name) = static_property_name_from_expression(property) {
        return object_binding
            .string_properties
            .iter()
            .find(|(existing_name, _)| *existing_name == property_name)
            .map(|(_, value)| value);
    }
    object_binding
        .symbol_properties
        .iter()
        .find(|(existing_key, _)| existing_key == property)
        .map(|(_, value)| value)
}

pub(in crate::backend::direct_wasm) fn object_binding_has_property(
    object_binding: &ObjectValueBinding,
    property: &Expression,
) -> bool {
    object_binding_lookup_value(object_binding, property).is_some()
}

pub(in crate::backend::direct_wasm) fn object_binding_set_property(
    object_binding: &mut ObjectValueBinding,
    property: Expression,
    value: Expression,
) {
    if let Some(property_name) = static_property_name_from_expression(&property) {
        if let Some((_, existing_value)) = object_binding
            .string_properties
            .iter_mut()
            .find(|(existing_name, _)| *existing_name == property_name)
        {
            *existing_value = value;
        } else {
            object_binding
                .string_properties
                .push((property_name.clone(), value));
        }
        object_binding_set_string_property_enumerable(object_binding, &property_name, true);
        return;
    }

    if let Some((_, existing_value)) = object_binding
        .symbol_properties
        .iter_mut()
        .find(|(existing_key, _)| *existing_key == property)
    {
        *existing_value = value;
    } else {
        object_binding.symbol_properties.push((property, value));
    }
}

pub(in crate::backend::direct_wasm) fn object_binding_define_property(
    object_binding: &mut ObjectValueBinding,
    property: Expression,
    value: Expression,
    enumerable: bool,
) {
    if let Some(property_name) = static_property_name_from_expression(&property) {
        if let Some((_, existing_value)) = object_binding
            .string_properties
            .iter_mut()
            .find(|(existing_name, _)| *existing_name == property_name)
        {
            *existing_value = value;
        } else {
            object_binding
                .string_properties
                .push((property_name.clone(), value));
        }
        object_binding_set_string_property_enumerable(object_binding, &property_name, enumerable);
        return;
    }

    if let Some((_, existing_value)) = object_binding
        .symbol_properties
        .iter_mut()
        .find(|(existing_key, _)| *existing_key == property)
    {
        *existing_value = value;
    } else {
        object_binding.symbol_properties.push((property, value));
    }
}

pub(in crate::backend::direct_wasm) fn object_binding_remove_property(
    object_binding: &mut ObjectValueBinding,
    property: &Expression,
) -> bool {
    if let Some(property_name) = static_property_name_from_expression(property) {
        let len_before = object_binding.string_properties.len();
        object_binding
            .string_properties
            .retain(|(existing_name, _)| *existing_name != property_name);
        object_binding
            .non_enumerable_string_properties
            .retain(|hidden_name| hidden_name != &property_name);
        return object_binding.string_properties.len() != len_before;
    }

    let len_before = object_binding.symbol_properties.len();
    object_binding
        .symbol_properties
        .retain(|(existing_key, _)| existing_key != property);
    object_binding.symbol_properties.len() != len_before
}

pub(in crate::backend::direct_wasm) fn merge_enumerable_object_binding(
    target: &mut ObjectValueBinding,
    source: &ObjectValueBinding,
) {
    for (name, value) in &source.string_properties {
        if source
            .non_enumerable_string_properties
            .iter()
            .any(|hidden_name| hidden_name == name)
        {
            continue;
        }
        object_binding_set_property(target, Expression::String(name.clone()), value.clone());
    }
    for (property, value) in &source.symbol_properties {
        object_binding_set_property(target, property.clone(), value.clone());
    }
}
