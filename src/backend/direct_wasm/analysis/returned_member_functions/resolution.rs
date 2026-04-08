use super::*;

pub(in crate::backend::direct_wasm) fn returned_member_function_binding_key(
    object: &Expression,
    property: &Expression,
    returned_identifier: &str,
) -> Option<ReturnedMemberFunctionBindingKey> {
    let Expression::String(property_name) = property else {
        return None;
    };

    let target = match object {
        Expression::Identifier(name) if name == returned_identifier => {
            ReturnedMemberFunctionBindingTarget::Value
        }
        Expression::Member { object, property }
            if matches!(property.as_ref(), Expression::String(name) if name == "prototype")
                && matches!(object.as_ref(), Expression::Identifier(name) if name == returned_identifier) =>
        {
            ReturnedMemberFunctionBindingTarget::Prototype
        }
        _ => return None,
    };

    Some(ReturnedMemberFunctionBindingKey {
        target,
        property: property_name.clone(),
    })
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_function_binding_from_descriptor(
    descriptor: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) -> Option<LocalFunctionBinding> {
    let Expression::Object(entries) = descriptor else {
        return None;
    };
    for entry in entries {
        let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
            continue;
        };
        if matches!(key, Expression::String(name) if name == "value") {
            return resolve_returned_member_function_binding(
                value,
                returned_identifier,
                function_names,
                bindings,
            );
        }
    }
    None
}

pub(in crate::backend::direct_wasm) fn resolve_returned_member_function_binding(
    expression: &Expression,
    returned_identifier: &str,
    function_names: &HashSet<String>,
    bindings: &HashMap<ReturnedMemberFunctionBindingKey, LocalFunctionBinding>,
) -> Option<LocalFunctionBinding> {
    match expression {
        Expression::Identifier(name) if function_names.contains(name) => {
            Some(LocalFunctionBinding::User(name.clone()))
        }
        Expression::Member { object, property } => {
            let key = returned_member_function_binding_key(object, property, returned_identifier)?;
            bindings.get(&key).cloned()
        }
        _ => None,
    }
}
