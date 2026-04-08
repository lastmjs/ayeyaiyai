use super::*;

impl<'a> FunctionCompiler<'a> {
    fn inherited_member_binding_target(
        &self,
        name: &str,
        target: ReturnedMemberFunctionBindingTarget,
    ) -> MemberFunctionBindingTarget {
        match target {
            ReturnedMemberFunctionBindingTarget::Value => {
                MemberFunctionBindingTarget::Identifier(name.to_string())
            }
            ReturnedMemberFunctionBindingTarget::Prototype => {
                MemberFunctionBindingTarget::Prototype(name.to_string())
            }
        }
    }

    fn insert_inherited_member_function_binding_for_name(
        &mut self,
        name: &str,
        binding: ReturnedMemberFunctionBinding,
        capture_slots_by_property: &HashMap<String, BTreeMap<String, String>>,
    ) {
        let property_name = binding.property.clone();
        let key = MemberFunctionBindingKey {
            target: self.inherited_member_binding_target(name, binding.target),
            property: MemberFunctionBindingProperty::String(property_name.clone()),
        };
        self.state
            .speculation
            .static_semantics
            .objects
            .member_function_bindings
            .insert(key.clone(), binding.binding.clone());
        if let Some(capture_slots) = capture_slots_by_property.get(&property_name).cloned() {
            self.state
                .speculation
                .static_semantics
                .objects
                .member_function_capture_slots
                .insert(key.clone(), capture_slots.clone());
            if self.binding_name_is_global(name) {
                self.backend
                    .set_global_member_function_capture_slots(key.clone(), capture_slots);
            }
        }
        if self.binding_name_is_global(name) {
            self.backend
                .set_global_member_function_binding(key, binding.binding);
        }
    }

    fn insert_inherited_member_getter_binding_for_name(
        &mut self,
        name: &str,
        binding: ReturnedMemberFunctionBinding,
        capture_slots_by_property: &HashMap<String, BTreeMap<String, String>>,
    ) {
        let property_name = binding.property.clone();
        let key = MemberFunctionBindingKey {
            target: self.inherited_member_binding_target(name, binding.target),
            property: MemberFunctionBindingProperty::String(property_name.clone()),
        };
        self.state
            .speculation
            .static_semantics
            .objects
            .member_getter_bindings
            .insert(key.clone(), binding.binding.clone());
        if let Some(capture_slots) = capture_slots_by_property.get(&property_name).cloned() {
            self.state
                .speculation
                .static_semantics
                .objects
                .member_function_capture_slots
                .insert(key.clone(), capture_slots.clone());
            if self.binding_name_is_global(name) {
                self.backend
                    .set_global_member_function_capture_slots(key.clone(), capture_slots);
            }
        }
        if self.binding_name_is_global(name) {
            self.backend
                .set_global_member_getter_binding(key, binding.binding);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_member_function_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
        value_local: u32,
    ) -> DirectResult<()> {
        self.clear_member_function_bindings_for_name(name);
        if let Expression::Identifier(source_name) = value {
            self.copy_member_bindings_for_alias(name, source_name);
            return Ok(());
        }

        let inherited_source = self
            .direct_iterator_binding_source_expression(value)
            .unwrap_or(value);
        if let Expression::Identifier(source_name) = inherited_source {
            self.copy_member_bindings_for_alias(name, source_name);
            return Ok(());
        }
        let inherited_function_bindings = self.inherited_member_function_bindings(inherited_source);
        let capture_slots_by_property = self
            .initialize_returned_member_capture_slots_for_bindings(
                name,
                inherited_source,
                value_local,
                &inherited_function_bindings,
            )?;
        for binding in inherited_function_bindings {
            self.insert_inherited_member_function_binding_for_name(
                name,
                binding,
                &capture_slots_by_property,
            );
        }
        let inherited_getter_bindings = self.inherited_member_getter_bindings(inherited_source);
        let getter_capture_slots_by_property = self
            .initialize_returned_member_capture_slots_for_bindings(
                name,
                inherited_source,
                value_local,
                &inherited_getter_bindings,
            )?;
        for binding in inherited_getter_bindings {
            self.insert_inherited_member_getter_binding_for_name(
                name,
                binding,
                &getter_capture_slots_by_property,
            );
        }
        if let Expression::GetIterator(iterated) = value {
            let iterator_call = Expression::Call {
                callee: Box::new(Expression::Member {
                    object: Box::new((**iterated).clone()),
                    property: Box::new(symbol_iterator_expression()),
                }),
                arguments: Vec::new(),
            };
            let iterator_function_bindings =
                self.inherited_member_function_bindings(&iterator_call);
            let iterator_capture_slots_by_property = self
                .initialize_returned_member_capture_slots_for_bindings(
                    name,
                    &iterator_call,
                    value_local,
                    &iterator_function_bindings,
                )?;
            for binding in iterator_function_bindings {
                self.insert_inherited_member_function_binding_for_name(
                    name,
                    binding,
                    &iterator_capture_slots_by_property,
                );
            }
            let iterator_getter_bindings = self.inherited_member_getter_bindings(&iterator_call);
            let iterator_getter_capture_slots_by_property = self
                .initialize_returned_member_capture_slots_for_bindings(
                    name,
                    &iterator_call,
                    value_local,
                    &iterator_getter_bindings,
                )?;
            for binding in iterator_getter_bindings {
                self.insert_inherited_member_getter_binding_for_name(
                    name,
                    binding,
                    &iterator_getter_capture_slots_by_property,
                );
            }
        }
        Ok(())
    }
}
