use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn member_function_binding_property(
        &self,
        property: &Expression,
    ) -> Option<MemberFunctionBindingProperty> {
        let resolved_property = self
            .resolve_bound_alias_expression(property)
            .filter(|resolved| !static_expression_matches(resolved, property))
            .unwrap_or_else(|| property.clone());
        let materialized_property = self.materialize_static_expression(&resolved_property);
        let coerced_property = self
            .resolve_property_key_expression(&resolved_property)
            .unwrap_or_else(|| materialized_property.clone());

        for candidate in [
            property,
            &resolved_property,
            &materialized_property,
            &coerced_property,
        ] {
            if let Some(property_name) = static_property_name_from_expression(candidate) {
                return Some(MemberFunctionBindingProperty::String(property_name));
            }
            if let Some(symbol_name) = self.well_known_symbol_name(candidate) {
                return Some(MemberFunctionBindingProperty::Symbol(symbol_name));
            }
            if let Some(Expression::Identifier(symbol_name)) =
                self.resolve_symbol_identity_expression(candidate)
            {
                return Some(MemberFunctionBindingProperty::Symbol(symbol_name));
            }
        }

        match &resolved_property {
            Expression::Call { callee, .. }
                if matches!(callee.as_ref(), Expression::Identifier(name)
                    if name == "Symbol" && self.is_unshadowed_builtin_identifier(name)) =>
            {
                Some(MemberFunctionBindingProperty::SymbolExpression(format!(
                    "{resolved_property:?}"
                )))
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn well_known_symbol_name(
        &self,
        expression: &Expression,
    ) -> Option<String> {
        let Expression::Member { object, property } = expression else {
            return None;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Symbol" && self.is_unshadowed_builtin_identifier(name))
        {
            return None;
        }
        let Expression::String(name) = property.as_ref() else {
            return None;
        };
        Some(format!("Symbol.{name}"))
    }
}
