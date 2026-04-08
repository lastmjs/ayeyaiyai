use super::*;

impl<'a> FunctionCompiler<'a> {
    fn rebound_member_target(
        &self,
        target: &MemberFunctionBindingTarget,
        name: &str,
        source_name: &str,
    ) -> Option<MemberFunctionBindingTarget> {
        match target {
            MemberFunctionBindingTarget::Identifier(target) if target == source_name => {
                Some(MemberFunctionBindingTarget::Identifier(name.to_string()))
            }
            MemberFunctionBindingTarget::Prototype(target) if target == source_name => {
                Some(MemberFunctionBindingTarget::Prototype(name.to_string()))
            }
            _ => None,
        }
    }

    pub(super) fn copy_member_bindings_for_alias(&mut self, name: &str, source_name: &str) {
        let mut function_bindings = Vec::new();
        let mut function_capture_slots = Vec::new();
        let mut getter_bindings = Vec::new();
        let mut setter_bindings = Vec::new();

        let local_function_bindings = self
            .state
            .speculation
            .static_semantics
            .objects
            .member_function_bindings
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()));
        let global_function_bindings = self.backend.global_member_function_binding_entries();
        for (key, binding) in local_function_bindings.chain(global_function_bindings) {
            let Some(target) = self.rebound_member_target(&key.target, name, source_name) else {
                continue;
            };
            let rebound_key = MemberFunctionBindingKey {
                target,
                property: key.property.clone(),
            };
            function_bindings.push((rebound_key.clone(), binding));
            if let Some(capture_slots) = self
                .state
                .speculation
                .static_semantics
                .objects
                .member_function_capture_slots
                .get(&key)
                .cloned()
                .or_else(|| {
                    self.backend
                        .global_member_function_capture_slots(&key)
                        .cloned()
                })
            {
                function_capture_slots.push((rebound_key, capture_slots));
            }
        }

        let local_getter_bindings = self
            .state
            .speculation
            .static_semantics
            .objects
            .member_getter_bindings
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()));
        let global_getter_bindings = self.backend.global_member_getter_binding_entries();
        for (key, binding) in local_getter_bindings.chain(global_getter_bindings) {
            let Some(target) = self.rebound_member_target(&key.target, name, source_name) else {
                continue;
            };
            getter_bindings.push((
                MemberFunctionBindingKey {
                    target,
                    property: key.property.clone(),
                },
                binding,
            ));
        }

        let local_setter_bindings = self
            .state
            .speculation
            .static_semantics
            .objects
            .member_setter_bindings
            .iter()
            .map(|(key, binding)| (key.clone(), binding.clone()));
        let global_setter_bindings = self.backend.global_member_setter_binding_entries();
        for (key, binding) in local_setter_bindings.chain(global_setter_bindings) {
            let Some(target) = self.rebound_member_target(&key.target, name, source_name) else {
                continue;
            };
            setter_bindings.push((
                MemberFunctionBindingKey {
                    target,
                    property: key.property.clone(),
                },
                binding,
            ));
        }

        for (key, binding) in function_bindings {
            self.state
                .speculation
                .static_semantics
                .objects
                .member_function_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.backend
                    .set_global_member_function_binding(key, binding);
            }
        }
        for (key, capture_slots) in function_capture_slots {
            self.state
                .speculation
                .static_semantics
                .objects
                .member_function_capture_slots
                .insert(key.clone(), capture_slots.clone());
            if self.binding_name_is_global(name) {
                self.backend
                    .set_global_member_function_capture_slots(key, capture_slots);
            }
        }
        for (key, binding) in getter_bindings {
            self.state
                .speculation
                .static_semantics
                .objects
                .member_getter_bindings
                .insert(key.clone(), binding.clone());
            if self.binding_name_is_global(name) {
                self.backend.set_global_member_getter_binding(key, binding);
            }
        }
        for (key, binding) in setter_bindings {
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
