use super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_value_bindings(
    statements: &[Statement],
) -> Vec<ReturnedMemberValueBinding> {
    if let Some(entries) = collect_returned_object_literal(statements) {
        return entries
            .into_iter()
            .filter_map(|entry| match entry {
                crate::ir::hir::ObjectEntry::Data {
                    key: Expression::String(property),
                    value,
                } => Some(ReturnedMemberValueBinding { property, value }),
                _ => None,
            })
            .collect();
    }

    let Some(returned_identifier) = collect_returned_identifier(statements) else {
        return Vec::new();
    };
    let local_aliases = collect_returned_member_local_aliases(statements);

    let mut bindings = HashMap::new();
    for statement in statements {
        collect_returned_member_value_bindings_from_statement(
            statement,
            &returned_identifier,
            &local_aliases,
            &mut bindings,
        );
    }

    bindings
        .into_iter()
        .map(|(property, value)| ReturnedMemberValueBinding { property, value })
        .collect()
}
