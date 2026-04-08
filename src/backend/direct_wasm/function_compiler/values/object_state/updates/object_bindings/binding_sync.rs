use super::super::super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_object_prototype_binding_from_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        if !self.binding_name_is_global(name) {
            return;
        }

        let materialized = self.materialize_static_expression(value);
        let prototype = object_literal_prototype_expression(&materialized).or_else(|| {
            let Expression::New { callee, .. } = &materialized else {
                return None;
            };
            let Expression::Identifier(constructor_name) = callee.as_ref() else {
                return None;
            };
            Some(Expression::Member {
                object: Box::new(Expression::Identifier(constructor_name.clone())),
                property: Box::new(Expression::String("prototype".to_string())),
            })
        });

        self.backend
            .sync_global_object_prototype_expression(name, prototype);
    }

    pub(in crate::backend::direct_wasm) fn update_local_object_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(object_binding) = self.resolve_object_binding_from_expression(value) else {
            self.state
                .speculation
                .static_semantics
                .clear_local_object_binding(name);
            if self.binding_name_is_global(name) {
                self.backend.sync_global_object_binding(name, None);
            }
            return;
        };
        self.state
            .speculation
            .static_semantics
            .set_local_object_binding(name, object_binding.clone());
        if self.binding_name_is_global(name) {
            self.backend
                .sync_global_object_binding(name, Some(object_binding));
        }
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_prototype_object_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Some(object_binding) = self.resolve_object_binding_from_expression(value) else {
            self.state
                .speculation
                .static_semantics
                .objects
                .local_prototype_object_bindings
                .remove(name);
            if self.binding_name_is_global(name) {
                self.backend
                    .sync_global_prototype_object_binding(name, None);
            }
            return;
        };
        self.state
            .speculation
            .static_semantics
            .objects
            .local_prototype_object_bindings
            .insert(name.to_string(), object_binding.clone());
        if self.binding_name_is_global(name) {
            self.backend
                .sync_global_prototype_object_binding(name, Some(object_binding));
        }
    }
}
