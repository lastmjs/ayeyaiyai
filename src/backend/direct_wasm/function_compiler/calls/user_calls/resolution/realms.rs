use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_test262_realm_id_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        match expression {
            Expression::Identifier(name) => {
                if let Some(realm_id) = parse_test262_realm_identifier(name) {
                    return Some(realm_id);
                }
                let resolved = self.resolve_bound_alias_expression(expression)?;
                let Expression::Identifier(name) = resolved else {
                    return None;
                };
                parse_test262_realm_identifier(&name)
            }
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_test262_realm_global_id_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<u32> {
        let materialized = self.materialize_static_expression(expression);
        match &materialized {
            Expression::Identifier(name) => parse_test262_realm_global_identifier(name),
            Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(name) if name == "global") => {
                self.resolve_test262_realm_id_from_expression(object)
            }
            _ => None,
        }
    }
}
