use super::*;

pub(in crate::backend::direct_wasm) fn scoped_binding_source_name(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("__ayy_scope$")?;
    let (source_name, scope_id) = rest.rsplit_once('$')?;
    scope_id
        .chars()
        .all(|character| character.is_ascii_digit())
        .then_some(source_name)
}

pub(in crate::backend::direct_wasm) fn is_eval_local_function_declaration_statement(
    statement: &Statement,
    declarations: &HashMap<String, String>,
) -> bool {
    let Statement::Let { name, value, .. } = statement else {
        return false;
    };
    let Expression::Identifier(function_name) = value else {
        return false;
    };
    declarations
        .get(name)
        .is_some_and(|expected| expected == function_name)
}
