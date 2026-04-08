use super::*;

pub(in crate::backend::direct_wasm) fn collect_eval_statement_var_names(
    statements: &[Statement],
) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_eval_var_names_from_statements(statements, &mut names);
    names
}
