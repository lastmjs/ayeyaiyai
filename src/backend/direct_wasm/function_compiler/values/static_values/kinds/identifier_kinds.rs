use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn infer_typeof_operand_kind(
        &self,
        expression: &Expression,
    ) -> Option<StaticValueKind> {
        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression) {
            return self.infer_typeof_operand_kind(&materialized);
        }
        match expression {
            Expression::Member { object, property } => {
                if let Some(getter_binding) =
                    self.resolve_member_getter_binding_shallow(object, property)
                {
                    if let Some(StaticEvalOutcome::Value(value)) = self
                        .resolve_static_function_outcome_from_binding_with_context(
                            &getter_binding,
                            &[],
                            self.current_function_name(),
                        )
                    {
                        return self.infer_typeof_operand_kind(&value);
                    }
                    return None;
                }
                self.infer_value_kind(expression)
            }
            Expression::This => self
                .state
                .speculation
                .execution_context
                .top_level_function
                .then_some(StaticValueKind::Object),
            Expression::Identifier(name)
                if name == "NaN" && self.is_unshadowed_builtin_identifier(name) =>
            {
                Some(StaticValueKind::Number)
            }
            Expression::Identifier(name) => self
                .lookup_identifier_kind(name)
                .or(Some(StaticValueKind::Undefined)),
            _ => self.infer_value_kind(expression),
        }
    }

    pub(in crate::backend::direct_wasm) fn lookup_identifier_kind(
        &self,
        name: &str,
    ) -> Option<StaticValueKind> {
        if self.with_scope_blocks_static_identifier_resolution(name) {
            return None;
        }
        if self.is_current_arguments_binding_name(name) && self.has_arguments_object() {
            return Some(StaticValueKind::Object);
        }
        if parse_test262_realm_identifier(name).is_some()
            || parse_test262_realm_global_identifier(name).is_some()
        {
            return Some(StaticValueKind::Object);
        }
        let identifier = Expression::Identifier(name.to_string());
        if let Some(resolved) = self.resolve_bound_alias_expression(&identifier)
            && !static_expression_matches(&resolved, &identifier)
            && let Some(kind) = self.infer_value_kind(&resolved)
            && kind != StaticValueKind::Unknown
        {
            return Some(kind);
        }
        if let Some((resolved_name, _)) = self.resolve_current_local_binding(name) {
            return Some(
                self.state
                    .speculation
                    .static_semantics
                    .local_kind(&resolved_name)
                    .unwrap_or(StaticValueKind::Unknown),
            );
        }
        if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(name)
            && let Some(kind) = self.global_binding_kind(&hidden_name)
        {
            return Some(kind);
        }
        if matches!(
            self.state
                .speculation
                .static_semantics
                .local_function_binding(name),
            Some(LocalFunctionBinding::User(_) | LocalFunctionBinding::Builtin(_))
        ) {
            return Some(StaticValueKind::Function);
        }
        if let Some(kind) = self.global_binding_kind(name) {
            return Some(kind);
        }
        if self.resolve_eval_local_function_hidden_name(name).is_some() {
            return Some(
                self.state
                    .speculation
                    .static_semantics
                    .local_kind(name)
                    .unwrap_or(StaticValueKind::Unknown),
            );
        }
        if self.global_has_binding(name) {
            return Some(StaticValueKind::Unknown);
        }
        if self
            .state
            .runtime
            .locals
            .deleted_builtin_identifiers
            .contains(name)
        {
            return None;
        }
        if is_internal_user_function_identifier(name)
            && self
                .backend
                .function_registry
                .catalog
                .user_function(name)
                .is_some()
        {
            return Some(StaticValueKind::Function);
        }
        builtin_identifier_kind(name)
    }
}
