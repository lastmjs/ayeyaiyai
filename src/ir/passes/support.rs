use std::collections::HashSet;

use crate::ir::hir::{CallArgument, Expression, Statement};

pub(super) fn collect_statement_bindings<'a>(
    statements: impl Iterator<Item = &'a Statement>,
) -> Vec<String> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    for statement in statements {
        if let Some(name) = statement.declared_binding_name()
            && seen.insert(name.to_string())
        {
            bindings.push(name.to_string());
        }
    }
    bindings
}

pub(super) fn function_constructor_literal_source_parts(
    arguments: &[CallArgument],
) -> Option<(String, String)> {
    let parts = arguments
        .iter()
        .map(|argument| match argument {
            CallArgument::Expression(Expression::String(text)) => Some(text.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    let Some((body_source, parameter_sources)) = parts.split_last() else {
        return Some((String::new(), String::new()));
    };

    Some((parameter_sources.join(","), body_source.clone()))
}
