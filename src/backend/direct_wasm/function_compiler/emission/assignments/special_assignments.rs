use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn prepare_special_assignment_expression(
        &mut self,
        expression: &Expression,
    ) -> Option<Expression> {
        match expression {
            Expression::Identifier(name) => {
                let resolved = self.resolve_bound_alias_expression(expression)?;
                let Expression::Identifier(resolved_name) = resolved else {
                    return None;
                };
                if resolved_name != *name
                    && (parse_test262_realm_identifier(&resolved_name).is_some()
                        || parse_test262_realm_global_identifier(&resolved_name).is_some())
                {
                    return Some(Expression::Identifier(resolved_name));
                }
                None
            }
            Expression::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        callee.as_ref(),
                        Expression::Member { object, property }
                            if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                                && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
                    ) =>
            {
                let realm_id = self.allocate_test262_realm();
                Some(Expression::Identifier(test262_realm_identifier(realm_id)))
            }
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "global") =>
            {
                let realm_expression = self
                    .prepare_special_assignment_expression(object)
                    .unwrap_or_else(|| self.materialize_static_expression(object));
                let Expression::Identifier(realm_name) = realm_expression else {
                    return None;
                };
                let realm_id = parse_test262_realm_identifier(&realm_name)?;
                Some(Expression::Identifier(test262_realm_global_identifier(
                    realm_id,
                )))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_registered_function_declaration(
        &self,
        function_name: &str,
    ) -> Option<&FunctionDeclaration> {
        self.prepared_function_declaration(function_name)
            .or_else(|| {
                self.backend
                    .function_registry
                    .registered_function(function_name)
            })
    }
}
