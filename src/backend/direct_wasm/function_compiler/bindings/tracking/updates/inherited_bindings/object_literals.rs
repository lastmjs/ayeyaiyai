use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_object_literal_member_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Object(entries) = value else {
            return;
        };

        self.clear_object_literal_member_bindings_for_name(name);

        let mut states: HashMap<
            MemberFunctionBindingProperty,
            (
                Option<LocalFunctionBinding>,
                Option<LocalFunctionBinding>,
                Option<LocalFunctionBinding>,
            ),
        > = HashMap::new();

        for entry in entries {
            let (key, binding, slot) = match entry {
                crate::ir::hir::ObjectEntry::Data { key, value } => {
                    (key, self.resolve_function_binding_from_expression(value), 0)
                }
                crate::ir::hir::ObjectEntry::Getter { key, getter } => (
                    key,
                    self.resolve_function_binding_from_expression(getter),
                    1,
                ),
                crate::ir::hir::ObjectEntry::Setter { key, setter } => (
                    key,
                    self.resolve_function_binding_from_expression(setter),
                    2,
                ),
                crate::ir::hir::ObjectEntry::Spread(_) => return,
            };

            let materialized_key = self
                .resolve_property_key_expression(key)
                .unwrap_or_else(|| self.materialize_static_expression(key));
            let Some(property_name) = self.member_function_binding_property(&materialized_key)
            else {
                continue;
            };
            let state = states.entry(property_name).or_insert((None, None, None));
            match slot {
                0 => {
                    state.0 = binding;
                    state.1 = None;
                    state.2 = None;
                }
                1 => {
                    state.0 = None;
                    state.1 = binding;
                }
                2 => {
                    state.0 = None;
                    state.2 = binding;
                }
                _ => {}
            }
        }

        for (property, (value_binding, getter_binding, setter_binding)) in states {
            let key = MemberFunctionBindingKey {
                target: MemberFunctionBindingTarget::Identifier(name.to_string()),
                property,
            };
            if let Some(binding) = value_binding {
                self.state
                    .speculation
                    .static_semantics
                    .objects
                    .member_function_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.backend
                        .set_global_member_function_binding(key.clone(), binding);
                }
            }
            if let Some(binding) = getter_binding {
                self.state
                    .speculation
                    .static_semantics
                    .objects
                    .member_getter_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.backend
                        .set_global_member_getter_binding(key.clone(), binding);
                }
            }
            if let Some(binding) = setter_binding {
                self.state
                    .speculation
                    .static_semantics
                    .objects
                    .member_setter_bindings
                    .insert(key.clone(), binding.clone());
                if self.binding_name_is_global(name) {
                    self.backend.set_global_member_setter_binding(key, binding);
                }
            }
        }
    }
}
