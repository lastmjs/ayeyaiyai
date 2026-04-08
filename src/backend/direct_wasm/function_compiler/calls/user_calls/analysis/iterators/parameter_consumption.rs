use super::*;

#[path = "parameter_consumption/expression_traversal.rs"]
mod expression_traversal;
#[path = "parameter_consumption/statement_traversal.rs"]
mod statement_traversal;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn user_function_parameter_iterator_consumption_indices(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<usize> {
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return HashSet::new();
        };
        let param_names = user_function.params.iter().cloned().collect::<HashSet<_>>();
        let mut consumed_names = HashSet::new();
        Self::collect_parameter_get_iterator_names_from_statements(
            &function.body,
            &param_names,
            &mut consumed_names,
        );
        user_function
            .params
            .iter()
            .enumerate()
            .filter_map(|(index, param_name)| consumed_names.contains(param_name).then_some(index))
            .collect()
    }
}
