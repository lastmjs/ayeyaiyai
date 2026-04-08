use super::*;

impl DirectWasmCompiler {
    fn normalize_prototype_parent_expression(&self, expression: &Expression) -> Expression {
        match expression {
            Expression::Identifier(name) => {
                let resolved = self
                    .resolve_global_identifier_expression(name)
                    .filter(|resolved| {
                        !matches!(resolved, Expression::Identifier(resolved_name) if resolved_name == name)
                    })
                    .unwrap_or_else(|| Expression::Identifier(name.clone()));
                match resolved {
                    Expression::Identifier(resolved_name) => {
                        let normalized_name = self
                            .find_global_identifier_binding_name(&resolved_name)
                            .or_else(|| self.find_global_user_function_binding_name(&resolved_name))
                            .unwrap_or(resolved_name);
                        Expression::Identifier(normalized_name)
                    }
                    other => other,
                }
            }
            Expression::Member { object, property } => Expression::Member {
                object: Box::new(self.normalize_prototype_parent_expression(object)),
                property: Box::new(self.materialize_global_expression(property)),
            },
            _ => self.materialize_global_expression(expression),
        }
    }

    pub(super) fn prototype_assignment_parent_expression(
        &self,
        value: &Expression,
    ) -> Option<Expression> {
        match value {
            Expression::Call { callee, arguments }
                if matches!(
                    callee.as_ref(),
                    Expression::Member { object, property }
                        if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                            && matches!(property.as_ref(), Expression::String(name) if name == "create")
                ) =>
            {
                let argument = arguments.first()?;
                let (CallArgument::Expression(parent) | CallArgument::Spread(parent)) = argument;
                Some(self.normalize_prototype_parent_expression(parent))
            }
            _ => {
                let materialized = self.materialize_global_expression(value);
                (!static_expression_matches(&materialized, value))
                    .then(|| self.prototype_assignment_parent_expression(&materialized))?
            }
        }
    }
}
