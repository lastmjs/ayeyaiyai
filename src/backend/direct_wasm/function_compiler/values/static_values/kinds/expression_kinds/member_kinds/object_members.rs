use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn infer_object_member_kind(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<StaticValueKind> {
        if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
            if matches!(property, Expression::String(name) if name == "length") {
                return Some(StaticValueKind::Number);
            }
            if let Some(index) = argument_index_from_expression(property) {
                return array_binding
                    .values
                    .get(index as usize)
                    .and_then(|value| value.as_ref())
                    .and_then(|value| self.infer_value_kind(value))
                    .or(Some(StaticValueKind::Undefined));
            }
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
            let materialized_property = self.materialize_static_expression(property);
            return object_binding_lookup_value(&object_binding, &materialized_property)
                .and_then(|value| self.infer_value_kind(value))
                .or(Some(StaticValueKind::Undefined));
        }
        if let Expression::String(_) = object {
            if matches!(property, Expression::String(name) if name == "length") {
                return Some(StaticValueKind::Number);
            }
            if argument_index_from_expression(property).is_some() {
                return Some(StaticValueKind::String);
            }
        }
        None
    }
}
