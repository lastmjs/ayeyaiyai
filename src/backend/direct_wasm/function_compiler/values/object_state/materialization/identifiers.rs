use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn materialize_identifier_expression(
        &self,
        name: &str,
        expression: &Expression,
    ) -> Expression {
        if self.with_scope_blocks_static_identifier_resolution(name) {
            return Expression::Identifier(name.to_string());
        }
        if name == "undefined" && self.is_unshadowed_builtin_identifier(name) {
            return Expression::Undefined;
        }
        if self
            .state
            .speculation
            .static_semantics
            .has_local_object_binding(name)
            || self
                .backend
                .global_semantics
                .values
                .object_bindings
                .contains_key(name)
            || self
                .state
                .speculation
                .static_semantics
                .objects
                .local_prototype_object_bindings
                .contains_key(name)
            || self
                .backend
                .global_semantics
                .values
                .prototype_object_bindings
                .contains_key(name)
        {
            return expression.clone();
        }
        if self
            .state
            .speculation
            .static_semantics
            .has_local_array_binding(name)
            || self
                .backend
                .global_semantics
                .values
                .array_bindings
                .contains_key(name)
            || self
                .state
                .speculation
                .static_semantics
                .has_local_typed_array_view_binding(name)
        {
            return expression.clone();
        }
        if let Some(symbol_identity) = self.resolve_symbol_identity_expression(expression) {
            return symbol_identity;
        }
        if self
            .state
            .speculation
            .static_semantics
            .local_value_binding(name)
            .or_else(|| self.global_value_binding(name))
            .is_some_and(|value| {
                matches!(
                    value,
                    Expression::Call { callee, .. }
                        if matches!(callee.as_ref(), Expression::Identifier(symbol_name)
                            if symbol_name == "Symbol"
                                && self.is_unshadowed_builtin_identifier(symbol_name))
                )
            })
        {
            return Expression::Identifier(name.to_string());
        }
        if let Some(resolved) = self.resolve_bound_alias_expression(expression) {
            if !static_expression_matches(&resolved, expression) {
                let mut referenced_names = HashSet::new();
                collect_referenced_binding_names_from_expression(&resolved, &mut referenced_names);
                if referenced_names.contains(name) {
                    return Expression::Identifier(name.to_string());
                }
                return self.materialize_static_expression(&resolved);
            }
        }
        expression.clone()
    }
}
