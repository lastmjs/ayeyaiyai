use super::*;

pub(in crate::backend::direct_wasm) fn resolve_returned_member_value_property_key(
    object: &Expression,
    property: &Expression,
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
) -> Option<Expression> {
    let resolved_property = resolve_returned_member_local_alias_expression(property, local_aliases);
    let property_key = match resolved_property {
        Expression::String(property_name) => Expression::String(property_name),
        _ => return None,
    };

    match object {
        Expression::Identifier(name) if name == returned_identifier => Some(property_key),
        _ => None,
    }
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_value_from_descriptor(
    descriptor: &Expression,
) -> Option<Expression> {
    resolve_property_descriptor_definition(descriptor)?.value
}
