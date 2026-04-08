use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn apply_object_set_prototype_of_update(&mut self, arguments: &[CallArgument]) {
        let [
            CallArgument::Expression(target),
            CallArgument::Expression(prototype_expression),
            ..,
        ] = arguments
        else {
            return;
        };
        let Some(target_name) = self.resolve_global_set_prototype_of_target_name(target) else {
            return;
        };
        let prototype = self
            .resolve_bound_alias_expression(prototype_expression)
            .filter(|resolved| !static_expression_matches(resolved, prototype_expression))
            .unwrap_or_else(|| self.materialize_static_expression(prototype_expression));
        let prototype = match prototype {
            Expression::Sequence(expressions) => {
                expressions.last().cloned().unwrap_or(Expression::Undefined)
            }
            _ => prototype,
        };
        self.backend
            .sync_global_object_prototype_expression(&target_name, Some(prototype));
    }

    fn resolve_global_set_prototype_of_target_name(&self, target: &Expression) -> Option<String> {
        let target_name = match target {
            Expression::Identifier(name) if self.binding_name_is_global(name) => name.clone(),
            _ => match self
                .resolve_bound_alias_expression(target)
                .filter(|resolved| !static_expression_matches(resolved, target))
                .unwrap_or_else(|| self.materialize_static_expression(target))
            {
                Expression::Identifier(name) => name,
                _ => match target {
                    Expression::Identifier(name) => name.clone(),
                    _ => return None,
                },
            },
        };
        self.binding_name_is_global(&target_name)
            .then_some(target_name)
    }
}
