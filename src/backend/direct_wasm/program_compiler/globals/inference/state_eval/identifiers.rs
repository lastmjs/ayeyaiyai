use super::super::*;
use super::ProgramStaticEvalContext;

impl StaticIdentifierMaterializationSource for ProgramStaticEvalContext<'_> {
    fn static_identifier_kind(&self, name: &str) -> Option<StaticValueKind> {
        self.binding_kind(name)
    }

    fn static_unshadowed_builtin_identifier(&self, name: &str) -> bool {
        !self.has_binding(name) && !self.has_lexical_binding(name)
    }

    fn preserves_static_object_identifier_binding(
        &self,
        value: &Expression,
        _is_local: bool,
    ) -> bool {
        matches!(value, Expression::Object(_) | Expression::Identifier(_))
    }
}
