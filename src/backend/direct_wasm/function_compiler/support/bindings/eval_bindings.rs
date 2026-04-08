use super::*;

pub(in crate::backend::direct_wasm) fn eval_program_declares_var_arguments(
    program: &Program,
) -> bool {
    eval_statements_declare_var_arguments(&program.statements)
}

pub(in crate::backend::direct_wasm) fn collect_direct_eval_lexical_binding_names(
    statements: &[Statement],
) -> Vec<String> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    for statement in statements {
        if let Statement::Let { name, .. } = statement
            && seen.insert(name.clone())
        {
            bindings.push(name.clone());
        }
    }
    bindings
}
