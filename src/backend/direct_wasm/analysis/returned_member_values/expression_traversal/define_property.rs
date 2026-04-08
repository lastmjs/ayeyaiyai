use super::super::*;

pub(super) fn collect_define_property_returned_member_value_binding(
    callee: &Expression,
    arguments: &[CallArgument],
    returned_identifier: &str,
    local_aliases: &HashMap<String, Expression>,
    bindings: &mut HashMap<String, Expression>,
) {
    let Expression::Member { object, property } = callee else {
        return;
    };
    if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
        return;
    }
    if !matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
        return;
    }
    let [
        CallArgument::Expression(target),
        CallArgument::Expression(property),
        CallArgument::Expression(descriptor),
        ..,
    ] = arguments
    else {
        return;
    };
    let Some(Expression::String(property_name)) = resolve_returned_member_value_property_key(
        target,
        property,
        returned_identifier,
        local_aliases,
    ) else {
        return;
    };
    let Some(value) = resolve_returned_member_value_from_descriptor(descriptor) else {
        bindings.remove(&property_name);
        return;
    };
    bindings.insert(property_name, value);
}
