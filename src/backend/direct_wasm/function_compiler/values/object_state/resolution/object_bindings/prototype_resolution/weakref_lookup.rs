use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_static_weakref_target_expression(
        &self,
        expression: &Expression,
    ) -> Option<Expression> {
        if let Some(resolved) = self
            .resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
        {
            return self.resolve_static_weakref_target_expression(&resolved);
        }
        let Expression::New { callee, arguments } = expression else {
            let materialized = self.materialize_static_expression(expression);
            if !static_expression_matches(&materialized, expression) {
                return self.resolve_static_weakref_target_expression(&materialized);
            }
            return None;
        };
        if !matches!(callee.as_ref(), Expression::Identifier(name) if name == "WeakRef") {
            return None;
        }
        match arguments.first() {
            Some(CallArgument::Expression(target)) | Some(CallArgument::Spread(target)) => {
                Some(target.clone())
            }
            None => Some(Expression::Undefined),
        }
    }
}
