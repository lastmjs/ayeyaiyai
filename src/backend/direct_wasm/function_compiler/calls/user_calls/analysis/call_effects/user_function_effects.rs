use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_user_function_call_effect_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
    ) -> HashSet<String> {
        let mut visited = HashSet::new();
        self.collect_user_function_call_effect_nonlocal_bindings_for_name(
            &user_function.name,
            &mut visited,
        )
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_call_effect_nonlocal_bindings_for_name(
        &self,
        function_name: &str,
        visited: &mut HashSet<String>,
    ) -> HashSet<String> {
        if !visited.insert(function_name.to_string()) {
            return HashSet::new();
        }
        let Some(user_function) = self.user_function(function_name) else {
            return HashSet::new();
        };
        let mut names = self.collect_user_function_assigned_nonlocal_bindings(user_function);
        if !self
            .user_function_parameter_iterator_consumption_indices(user_function)
            .is_empty()
        {
            return names;
        }
        let Some(function) = self.resolve_registered_function_declaration(function_name) else {
            return names;
        };
        for statement in &function.body {
            self.collect_statement_call_effect_nonlocal_bindings(
                statement,
                Some(function_name),
                &mut names,
                visited,
            );
        }
        names
    }

    pub(in crate::backend::direct_wasm) fn collect_user_function_argument_call_effect_nonlocal_bindings(
        &self,
        user_function: &UserFunction,
        arguments: &[Expression],
    ) -> HashSet<String> {
        if !self
            .user_function_parameter_iterator_consumption_indices(user_function)
            .is_empty()
        {
            return HashSet::new();
        }
        let Some(function) = self.resolve_registered_function_declaration(&user_function.name)
        else {
            return HashSet::new();
        };
        let call_arguments = arguments
            .iter()
            .cloned()
            .map(CallArgument::Expression)
            .collect::<Vec<_>>();
        let mut iterator_names = Vec::new();
        Self::collect_iterator_close_binding_names_from_statements(
            &function.body,
            &mut iterator_names,
        );
        let mut names = HashSet::new();
        let mut visited = HashSet::new();
        for iterator_name in iterator_names {
            let Some(iterated) =
                Self::find_iterator_source_expression_in_statements(&function.body, &iterator_name)
            else {
                continue;
            };
            let iterated = self.substitute_user_function_argument_bindings(
                &iterated,
                user_function,
                &call_arguments,
            );
            let iterator_call = Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new(iterated),
                    property: Box::new(symbol_iterator_expression()),
                }),
                arguments: Vec::new(),
            };
            let Some(LocalFunctionBinding::User(function_name)) = self
                .inherited_member_function_bindings(&iterator_call)
                .into_iter()
                .find(|binding| binding.property == "return")
                .map(|binding| binding.binding)
            else {
                continue;
            };
            names.extend(
                self.collect_user_function_call_effect_nonlocal_bindings_for_name(
                    &function_name,
                    &mut visited,
                ),
            );
        }
        names
    }

    pub(in crate::backend::direct_wasm) fn invalidate_user_function_call_effect_nonlocal_bindings_except(
        &mut self,
        user_function: &UserFunction,
        preserved_names: &HashSet<String>,
    ) {
        let names = self
            .collect_user_function_call_effect_nonlocal_bindings(user_function)
            .difference(preserved_names)
            .cloned()
            .collect::<HashSet<_>>();
        if !names.is_empty() {
            let preserved_kinds = names
                .iter()
                .filter_map(|name| {
                    self.lookup_identifier_kind(name)
                        .map(|kind| (name.clone(), kind))
                })
                .collect::<HashMap<_, _>>();
            self.invalidate_static_binding_metadata_for_names_with_preserved_kinds(
                &names,
                &preserved_kinds,
            );
        }
    }
}
