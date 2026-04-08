use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_iterator_close_return_binding_in_function(
        &self,
        iterator_name: &str,
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        let function_name = current_function_name?;
        let function = self.resolve_registered_function_declaration(function_name)?;
        let iterated =
            Self::find_iterator_source_expression_in_statements(&function.body, iterator_name)?;
        let iterator_call = Expression::Call {
            callee: Box::new(Expression::Member {
                object: Box::new(iterated),
                property: Box::new(symbol_iterator_expression()),
            }),
            arguments: Vec::new(),
        };
        self.inherited_member_function_bindings(&iterator_call)
            .into_iter()
            .find(|binding| binding.property == "return")
            .map(|binding| binding.binding)
    }
}
