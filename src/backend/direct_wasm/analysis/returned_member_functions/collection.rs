use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_function_bindings(
    statements: &[Statement],
    function_names: &HashSet<String>,
) -> Vec<ReturnedMemberFunctionBinding> {
    if let Some(Expression::Object(entries)) =
        collect_returned_identifier_source_expression(statements)
    {
        return entries
            .into_iter()
            .filter_map(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data {
                    key: Expression::String(property),
                    value,
                } => resolve_returned_member_function_binding(
                    &value,
                    "",
                    function_names,
                    &HashMap::new(),
                )
                .map(|binding| ReturnedMemberFunctionBinding {
                    target: ReturnedMemberFunctionBindingTarget::Value,
                    property,
                    binding,
                }),
                _ => None,
            })
            .collect();
    }

    let Some(returned_identifier) = collect_returned_identifier(statements) else {
        return Vec::new();
    };

    let mut bindings = HashMap::new();
    for statement in statements {
        collect_returned_member_function_bindings_from_statement(
            statement,
            &returned_identifier,
            function_names,
            &mut bindings,
        );
    }

    bindings
        .into_iter()
        .map(|(key, binding)| ReturnedMemberFunctionBinding {
            target: key.target,
            property: key.property,
            binding,
        })
        .collect()
}
