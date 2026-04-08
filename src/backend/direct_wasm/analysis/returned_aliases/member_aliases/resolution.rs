use super::super::*;

pub(in crate::backend::direct_wasm) fn resolve_returned_member_local_alias_expression(
    expression: &Expression,
    aliases: &HashMap<String, Expression>,
) -> Expression {
    let mut current = expression;
    let mut visited = HashSet::new();
    loop {
        let Expression::Identifier(name) = current else {
            return current.clone();
        };
        if !visited.insert(name.clone()) {
            return expression.clone();
        }
        let Some(next) = aliases.get(name) else {
            return current.clone();
        };
        current = next;
    }
}
