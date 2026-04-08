use super::*;

pub(in crate::backend::direct_wasm) trait StaticIdentifierMaterializer {
    fn lookup_static_identifier_kind(&self, name: &str) -> Option<StaticValueKind>;

    fn is_unshadowed_builtin_identifier(&self, name: &str) -> bool;

    fn preserves_symbol_identifier(&self, name: &str) -> bool {
        self.lookup_static_identifier_kind(name) == Some(StaticValueKind::Symbol)
    }

    fn preserves_undefined_identifier(&self, name: &str) -> bool {
        name == "undefined" && self.is_unshadowed_builtin_identifier(name)
    }

    fn preserves_symbol_call_binding(&self, value: &Expression) -> bool {
        matches!(
            value,
            Expression::Call { callee, .. }
                if matches!(callee.as_ref(), Expression::Identifier(symbol_name)
                    if symbol_name == "Symbol"
                        && self.is_unshadowed_builtin_identifier(symbol_name))
        )
    }

    fn preserves_object_identifier_binding(&self, value: &Expression, is_local: bool) -> bool;
}

pub(in crate::backend::direct_wasm) trait StaticIdentifierMaterializationSource {
    fn static_identifier_kind(&self, name: &str) -> Option<StaticValueKind>;

    fn static_unshadowed_builtin_identifier(&self, name: &str) -> bool;

    fn preserves_static_object_identifier_binding(
        &self,
        value: &Expression,
        is_local: bool,
    ) -> bool;
}

impl<T> StaticIdentifierMaterializer for T
where
    T: StaticIdentifierMaterializationSource + ?Sized,
{
    fn lookup_static_identifier_kind(&self, name: &str) -> Option<StaticValueKind> {
        self.static_identifier_kind(name)
    }

    fn is_unshadowed_builtin_identifier(&self, name: &str) -> bool {
        self.static_unshadowed_builtin_identifier(name)
    }

    fn preserves_object_identifier_binding(&self, value: &Expression, is_local: bool) -> bool {
        self.preserves_static_object_identifier_binding(value, is_local)
    }
}
