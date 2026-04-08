use super::super::*;
use super::FunctionStaticEvalContext;

impl StaticIdentifierMaterializationSource for FunctionStaticEvalContext<'_, '_> {
    fn static_identifier_kind(&self, name: &str) -> Option<StaticValueKind> {
        self.lookup_identifier_kind(name)
    }

    fn static_unshadowed_builtin_identifier(&self, name: &str) -> bool {
        self.is_unshadowed_builtin_identifier(name)
    }

    fn preserves_static_object_identifier_binding(
        &self,
        value: &Expression,
        is_local: bool,
    ) -> bool {
        if is_local {
            matches!(
                value,
                Expression::Object(_) | Expression::Identifier(_) | Expression::New { .. }
            )
        } else {
            matches!(value, Expression::Object(_) | Expression::Identifier(_))
        }
    }
}
