use super::super::*;

pub(in crate::backend::direct_wasm) fn collect_returned_member_local_aliases(
    statements: &[Statement],
) -> HashMap<String, Expression> {
    let mut aliases = HashMap::new();
    for statement in statements {
        super::collect_returned_member_local_aliases_from_statement(statement, &mut aliases);
    }
    aliases
}
