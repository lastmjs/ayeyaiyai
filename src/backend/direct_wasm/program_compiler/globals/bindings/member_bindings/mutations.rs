use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn insert_global_inherited_member_function_binding_for_name(
        &mut self,
        name: &str,
        binding: ReturnedMemberFunctionBinding,
        capture_slots_by_property: &HashMap<String, BTreeMap<String, String>>,
    ) {
        let property_name = binding.property.clone();
        let target = match binding.target {
            ReturnedMemberFunctionBindingTarget::Value => {
                MemberFunctionBindingTarget::Identifier(name.to_string())
            }
            ReturnedMemberFunctionBindingTarget::Prototype => {
                MemberFunctionBindingTarget::Prototype(name.to_string())
            }
        };
        let key = MemberFunctionBindingKey {
            target,
            property: MemberFunctionBindingProperty::String(property_name.clone()),
        };
        if let Some(capture_slots) = capture_slots_by_property.get(&property_name).cloned() {
            self.set_global_member_function_capture_slots(key.clone(), capture_slots);
        }
        self.set_global_member_function_binding(key, binding.binding);
    }

    pub(in crate::backend::direct_wasm) fn insert_global_inherited_member_getter_binding_for_name(
        &mut self,
        name: &str,
        binding: ReturnedMemberFunctionBinding,
        capture_slots_by_property: &HashMap<String, BTreeMap<String, String>>,
    ) {
        let property_name = binding.property.clone();
        let target = match binding.target {
            ReturnedMemberFunctionBindingTarget::Value => {
                MemberFunctionBindingTarget::Identifier(name.to_string())
            }
            ReturnedMemberFunctionBindingTarget::Prototype => {
                MemberFunctionBindingTarget::Prototype(name.to_string())
            }
        };
        let key = MemberFunctionBindingKey {
            target,
            property: MemberFunctionBindingProperty::String(property_name.clone()),
        };
        if let Some(capture_slots) = capture_slots_by_property.get(&property_name).cloned() {
            self.set_global_member_function_capture_slots(key.clone(), capture_slots);
        }
        self.set_global_member_getter_binding(key, binding.binding);
    }

    pub(in crate::backend::direct_wasm) fn update_global_object_literal_member_bindings_for_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Object(entries) = value else {
            self.clear_global_member_bindings_for_name(name);
            return;
        };

        self.clear_global_member_bindings_for_name(name);

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
                ObjectEntry::Data { key, value } => {
                    (key, self.infer_global_function_binding(value), 0)
                }
                ObjectEntry::Getter { key, getter } => {
                    (key, self.infer_global_function_binding(getter), 1)
                }
                ObjectEntry::Setter { key, setter } => {
                    (key, self.infer_global_function_binding(setter), 2)
                }
                ObjectEntry::Spread(_) => return,
            };

            let Some(property) = self.infer_global_member_function_binding_property(key) else {
                continue;
            };
            let state = states.entry(property).or_insert((None, None, None));
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
                self.set_global_member_function_binding(key.clone(), binding);
            }
            if let Some(binding) = getter_binding {
                self.set_global_member_getter_binding(key.clone(), binding);
            }
            if let Some(binding) = setter_binding {
                self.set_global_member_setter_binding(key, binding);
            }
        }
    }
}
