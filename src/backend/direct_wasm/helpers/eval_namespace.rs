fn stable_eval_namespace_hash(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(super) fn eval_program_function_namespace(
    current_function_name: Option<&str>,
    source: &str,
) -> String {
    let context_hash = stable_eval_namespace_hash(current_function_name.unwrap_or("__top__"));
    let source_hash = stable_eval_namespace_hash(source);
    format!("__evalctx_{context_hash:016x}_{source_hash:016x}")
}

pub(in crate::backend::direct_wasm) fn namespaced_internal_eval_function_name(
    function_name: &str,
    namespace: &str,
) -> String {
    if let Some((prefix, hint)) = function_name.rsplit_once("__name_") {
        return format!("{prefix}__{namespace}__name_{hint}");
    }
    format!("{function_name}__{namespace}")
}
