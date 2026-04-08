use super::super::*;
use super::FunctionStaticEvalContext;

impl StaticMaterializationPolicySource for FunctionStaticEvalContext<'_, '_> {
    fn static_preserve_new_expressions_in_materialization(&self) -> bool {
        true
    }

    fn static_preserve_call_expressions_in_materialization(&self) -> bool {
        true
    }

    fn static_materialize_post_structural_fallback_expression(
        &self,
        expression: &Expression,
        _environment: &Self::Environment,
    ) -> Option<Expression> {
        Some(self.materialize_expression(expression))
    }
}
