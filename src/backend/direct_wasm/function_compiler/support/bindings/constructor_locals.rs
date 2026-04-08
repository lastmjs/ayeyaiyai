use super::*;

pub(in crate::backend::direct_wasm) fn collect_function_constructor_local_bindings(
    function: &FunctionDeclaration,
) -> HashSet<String> {
    let mut bindings = collect_declared_bindings_from_statements_recursive(&function.body);
    bindings.extend(
        function
            .params
            .iter()
            .map(|parameter| parameter.name.clone()),
    );
    if let Some(self_binding) = &function.self_binding {
        bindings.insert(self_binding.clone());
    }
    bindings.insert("arguments".to_string());
    bindings
}
