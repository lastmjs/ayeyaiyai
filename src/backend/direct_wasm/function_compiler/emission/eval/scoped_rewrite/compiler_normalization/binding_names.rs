use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn rewrite_eval_scoped_binding_name(
        name: &mut String,
        declared_bindings: &HashSet<String>,
        eval_local_function_bindings: &HashSet<String>,
    ) {
        if let Some(source_name) = scoped_binding_source_name(name) {
            if eval_local_function_bindings.contains(name) || !declared_bindings.contains(name) {
                *name = source_name.to_string();
            }
        }
    }
}
