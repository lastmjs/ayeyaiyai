use super::*;

pub(in crate::backend::direct_wasm) fn function_constructor_literal_source_parts(
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

pub(in crate::backend::direct_wasm) fn function_constructor_wrapper_sources(
    name: &str,
    parameter_source: &str,
    body_source: &str,
) -> Option<Vec<String>> {
    let wrap = |prefix: &str| -> String {
        format!("{prefix} __ayy_ctor({parameter_source}) {{\n{body_source}\n}}")
    };

    match name {
        "Function" => Some(vec![wrap("function")]),
        "AsyncFunction" => Some(vec![wrap("async function")]),
        "GeneratorFunction" => Some(vec![wrap("function*")]),
        "AsyncGeneratorFunction" => Some(vec![wrap("async function*")]),
        FUNCTION_CONSTRUCTOR_FAMILY_BUILTIN => Some(vec![
            wrap("function"),
            wrap("async function"),
            wrap("function*"),
            wrap("async function*"),
        ]),
        _ => None,
    }
}
