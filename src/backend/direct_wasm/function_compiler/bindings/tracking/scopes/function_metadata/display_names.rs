use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_user_function_display_name(
        &self,
        function_name: &str,
    ) -> Option<String> {
        if let Some(function) = self.resolve_registered_function_declaration(function_name)
            && let Some(display_name) = function_display_name(function)
        {
            return Some(display_name);
        }

        if let Some(display_name) = self
            .state
            .speculation
            .static_semantics
            .objects
            .member_getter_bindings
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()))
            .chain(self.backend.global_member_getter_binding_entries())
            .find_map(|(key, binding)| {
                matches!(&binding, LocalFunctionBinding::User(name) if name == function_name)
                    .then(|| {
                        self.member_function_property_display_name(&key.property, Some("get "))
                    })
                    .flatten()
            })
        {
            return Some(display_name);
        }

        if let Some(display_name) = self
            .state
            .speculation
            .static_semantics
            .objects
            .member_setter_bindings
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()))
            .chain(self.backend.global_member_setter_binding_entries())
            .find_map(|(key, binding)| {
                matches!(&binding, LocalFunctionBinding::User(name) if name == function_name)
                    .then(|| {
                        self.member_function_property_display_name(&key.property, Some("set "))
                    })
                    .flatten()
            })
        {
            return Some(display_name);
        }

        if let Some(display_name) = self
            .state
            .speculation
            .static_semantics
            .objects
            .member_function_bindings
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()))
            .chain(self.backend.global_member_function_binding_entries())
            .find_map(|(key, binding)| {
                matches!(&binding, LocalFunctionBinding::User(name) if name == function_name)
                    .then(|| self.member_function_property_display_name(&key.property, None))
                    .flatten()
            })
        {
            return Some(display_name);
        }

        None
    }

    fn member_function_property_display_name(
        &self,
        property: &MemberFunctionBindingProperty,
        prefix: Option<&str>,
    ) -> Option<String> {
        let base_name = match property {
            MemberFunctionBindingProperty::String(name) => Some(name.clone()),
            MemberFunctionBindingProperty::Symbol(name) => {
                self.symbol_function_name_fragment(&Expression::Identifier(name.clone()))
            }
            MemberFunctionBindingProperty::SymbolExpression(_) => None,
        }?;

        Some(match prefix {
            Some(prefix) => format!("{prefix}{base_name}"),
            None => base_name,
        })
    }

    fn symbol_function_name_fragment(&self, expression: &Expression) -> Option<String> {
        if let Expression::Identifier(name) = expression
            && let Some(value) = self
                .state
                .speculation
                .static_semantics
                .local_value_binding(name)
                .or_else(|| self.global_value_binding(name))
            && !static_expression_matches(value, expression)
        {
            return self.symbol_function_name_fragment(value);
        }

        let current_function_name = self.current_function_name();
        let symbol_text = self
            .resolve_static_symbol_to_string_value_with_context(expression, current_function_name)
            .or_else(|| {
                let materialized = self.materialize_static_expression(expression);
                (!static_expression_matches(&materialized, expression)).then(|| {
                    self.resolve_static_symbol_to_string_value_with_context(
                        &materialized,
                        current_function_name,
                    )
                })?
            })?;
        if let Some(description) = symbol_text
            .strip_prefix("Symbol(")
            .and_then(|suffix| suffix.strip_suffix(')'))
        {
            if description.is_empty() {
                return Some(String::new());
            }
            return Some(format!("[{description}]"));
        }
        Some(format!("[{symbol_text}]"))
    }
}
