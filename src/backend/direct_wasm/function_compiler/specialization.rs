use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn collect_capture_bindings_from_expression(
        &self,
        expression: &Expression,
        bindings: &mut BTreeSet<String>,
    ) {
        match expression {
            Expression::Identifier(name) => {
                if self.locals.contains_key(name) {
                    bindings.insert(name.clone());
                }
            }
            Expression::Member { object, property } => {
                self.collect_capture_bindings_from_expression(object, bindings);
                self.collect_capture_bindings_from_expression(property, bindings);
            }
            Expression::SuperMember { property } => {
                self.collect_capture_bindings_from_expression(property, bindings);
            }
            Expression::Assign { value, .. }
            | Expression::Await(value)
            | Expression::EnumerateKeys(value)
            | Expression::GetIterator(value)
            | Expression::IteratorClose(value)
            | Expression::Unary {
                expression: value, ..
            } => self.collect_capture_bindings_from_expression(value, bindings),
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                self.collect_capture_bindings_from_expression(object, bindings);
                self.collect_capture_bindings_from_expression(property, bindings);
                self.collect_capture_bindings_from_expression(value, bindings);
            }
            Expression::AssignSuperMember { property, value } => {
                self.collect_capture_bindings_from_expression(property, bindings);
                self.collect_capture_bindings_from_expression(value, bindings);
            }
            Expression::Binary { left, right, .. } => {
                self.collect_capture_bindings_from_expression(left, bindings);
                self.collect_capture_bindings_from_expression(right, bindings);
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.collect_capture_bindings_from_expression(condition, bindings);
                self.collect_capture_bindings_from_expression(then_expression, bindings);
                self.collect_capture_bindings_from_expression(else_expression, bindings);
            }
            Expression::Sequence(expressions) => {
                for expression in expressions {
                    self.collect_capture_bindings_from_expression(expression, bindings);
                }
            }
            Expression::Call { callee, arguments }
            | Expression::SuperCall { callee, arguments }
            | Expression::New { callee, arguments } => {
                self.collect_capture_bindings_from_expression(callee, bindings);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) | CallArgument::Spread(expression) => {
                            self.collect_capture_bindings_from_expression(expression, bindings);
                        }
                    }
                }
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.collect_capture_bindings_from_expression(expression, bindings);
                        }
                    }
                }
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.collect_capture_bindings_from_expression(key, bindings);
                            self.collect_capture_bindings_from_expression(value, bindings);
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.collect_capture_bindings_from_expression(key, bindings);
                            self.collect_capture_bindings_from_expression(getter, bindings);
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.collect_capture_bindings_from_expression(key, bindings);
                            self.collect_capture_bindings_from_expression(setter, bindings);
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.collect_capture_bindings_from_expression(expression, bindings);
                        }
                    }
                }
            }
            Expression::Number(_)
            | Expression::BigInt(_)
            | Expression::String(_)
            | Expression::Bool(_)
            | Expression::Null
            | Expression::Undefined
            | Expression::NewTarget
            | Expression::This
            | Expression::Sent
            | Expression::Update { .. } => {}
        }
    }

    pub(in crate::backend::direct_wasm) fn collect_capture_bindings_from_summary(
        &self,
        summary: &InlineFunctionSummary,
    ) -> BTreeSet<String> {
        let mut bindings = BTreeSet::new();
        for effect in &summary.effects {
            match effect {
                InlineFunctionEffect::Assign { value, .. } => {
                    self.collect_capture_bindings_from_expression(value, &mut bindings);
                }
                InlineFunctionEffect::Update { .. } => {}
                InlineFunctionEffect::Expression(expression) => {
                    self.collect_capture_bindings_from_expression(expression, &mut bindings);
                }
            }
        }
        if let Some(return_value) = summary.return_value.as_ref() {
            self.collect_capture_bindings_from_expression(return_value, &mut bindings);
        }
        bindings
    }

    pub(in crate::backend::direct_wasm) fn resolve_specialized_function_value_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<SpecializedFunctionValue> {
        match expression {
            Expression::Identifier(name) => self
                .local_specialized_function_values
                .get(name)
                .cloned()
                .or_else(|| {
                    self.module
                        .global_specialized_function_values
                        .get(name)
                        .cloned()
                }),
            _ => None,
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_value_template_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<SpecializedFunctionValue> {
        let binding = self.resolve_function_binding_from_expression(expression)?;
        let LocalFunctionBinding::User(function_name) = &binding else {
            return None;
        };
        let user_function = self.module.user_function_map.get(function_name)?;
        if user_function.is_async()
            || user_function.is_generator()
            || user_function.has_parameter_defaults()
        {
            return None;
        }
        if !user_function.extra_argument_indices.is_empty() {
            return None;
        }
        let summary = user_function.inline_summary.as_ref()?;
        if inline_summary_mentions_call_frame_state(summary) && !user_function.lexical_this {
            return None;
        }
        Some(SpecializedFunctionValue {
            binding,
            summary: summary.clone(),
        })
    }

    pub(in crate::backend::direct_wasm) fn instantiate_specialized_function_value(
        &mut self,
        template: &SpecializedFunctionValue,
    ) -> DirectResult<Option<SpecializedFunctionValue>> {
        let captured = self.collect_capture_bindings_from_summary(&template.summary);
        if captured.is_empty() {
            return Ok(None);
        }

        let mut bindings = HashMap::new();
        for name in captured {
            let hidden_name = self.allocate_named_hidden_local(
                "capture",
                self.lookup_identifier_kind(&name)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            self.emit_numeric_expression(&Expression::Identifier(name.clone()))?;
            let hidden_local = self
                .locals
                .get(&hidden_name)
                .copied()
                .expect("hidden capture local should be allocated");
            self.push_local_set(hidden_local);
            self.alias_runtime_binding_metadata(&hidden_name, &name);
            bindings.insert(name, Expression::Identifier(hidden_name));
        }

        Ok(Some(SpecializedFunctionValue {
            binding: template.binding.clone(),
            summary: rewrite_inline_function_summary_bindings(&template.summary, &bindings),
        }))
    }

    pub(in crate::backend::direct_wasm) fn alias_runtime_binding_metadata(
        &mut self,
        target: &str,
        source: &str,
    ) {
        if let Some(function_binding) = self.local_function_bindings.get(source).cloned() {
            self.local_function_bindings
                .insert(target.to_string(), function_binding);
        }
        if let Some(specialized) = self.local_specialized_function_values.get(source).cloned() {
            self.local_specialized_function_values
                .insert(target.to_string(), specialized);
        }
        if let Some(array_binding) = self.local_array_bindings.get(source).cloned() {
            self.local_array_bindings
                .insert(target.to_string(), array_binding);
        }
        if let Some(length_local) = self.runtime_array_length_locals.get(source).copied() {
            self.runtime_array_length_locals
                .insert(target.to_string(), length_local);
        }
        if let Some(slots) = self.runtime_array_slots.get(source).cloned() {
            self.runtime_array_slots.insert(target.to_string(), slots);
        }
        if let Some(bindings) = self.tracked_array_function_values.get(source).cloned() {
            self.tracked_array_function_values
                .insert(target.to_string(), bindings);
        }
        if let Some(object_binding) = self.local_object_bindings.get(source).cloned() {
            self.local_object_bindings
                .insert(target.to_string(), object_binding);
        }
        if let Some(arguments_binding) = self.local_arguments_bindings.get(source).cloned() {
            self.local_arguments_bindings
                .insert(target.to_string(), arguments_binding);
        }
        if self.direct_arguments_aliases.contains(source) {
            self.direct_arguments_aliases.insert(target.to_string());
        }
        if let Some(descriptor) = self.local_descriptor_bindings.get(source).cloned() {
            self.local_descriptor_bindings
                .insert(target.to_string(), descriptor);
        }
        if let Some(buffer_binding) = self
            .local_resizable_array_buffer_bindings
            .get(source)
            .cloned()
        {
            self.local_resizable_array_buffer_bindings
                .insert(target.to_string(), buffer_binding);
        }
        if let Some(view_binding) = self.local_typed_array_view_bindings.get(source).cloned() {
            self.local_typed_array_view_bindings
                .insert(target.to_string(), view_binding);
        }
        if let Some(oob_local) = self.runtime_typed_array_oob_locals.get(source).copied() {
            self.runtime_typed_array_oob_locals
                .insert(target.to_string(), oob_local);
        }
        if let Some(iterator_binding) = self.local_array_iterator_bindings.get(source).cloned() {
            self.local_array_iterator_bindings
                .insert(target.to_string(), iterator_binding);
        }
        if let Some(step_binding) = self.local_iterator_step_bindings.get(source).cloned() {
            self.local_iterator_step_bindings
                .insert(target.to_string(), step_binding);
        }
        if let Some(kind) = self.local_kinds.get(source).copied() {
            self.local_kinds.insert(target.to_string(), kind);
        }
    }

    pub(in crate::backend::direct_wasm) fn update_local_specialized_function_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        self.local_specialized_function_values.remove(name);
        if let Some(specialized) = self.resolve_specialized_function_value_from_expression(value) {
            self.local_specialized_function_values
                .insert(name.to_string(), specialized);
            return Ok(());
        }
        let Some(template) = self.resolve_function_value_template_from_expression(value) else {
            return Ok(());
        };
        let Some(specialized) = self.instantiate_specialized_function_value(&template)? else {
            return Ok(());
        };
        self.local_specialized_function_values
            .insert(name.to_string(), specialized);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn update_global_specialized_function_value(
        &mut self,
        name: &str,
        value: &Expression,
    ) -> DirectResult<()> {
        self.module.global_specialized_function_values.remove(name);
        if let Some(specialized) = self.resolve_specialized_function_value_from_expression(value) {
            self.module
                .global_specialized_function_values
                .insert(name.to_string(), specialized);
            return Ok(());
        }
        let Some(template) = self.resolve_function_value_template_from_expression(value) else {
            return Ok(());
        };
        let Some(specialized) = self.instantiate_specialized_function_value(&template)? else {
            return Ok(());
        };
        self.module
            .global_specialized_function_values
            .insert(name.to_string(), specialized);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn update_tracked_array_specialized_function_value(
        &mut self,
        name: &str,
        index: u32,
        value: &Expression,
    ) -> DirectResult<()> {
        self.tracked_array_function_values
            .entry(name.to_string())
            .or_default()
            .remove(&index);
        if let Some(specialized) = self.resolve_specialized_function_value_from_expression(value) {
            self.tracked_array_function_values
                .entry(name.to_string())
                .or_default()
                .insert(index, specialized);
            return Ok(());
        }
        let Some(template) = self.resolve_function_value_template_from_expression(value) else {
            return Ok(());
        };
        let Some(specialized) = self.instantiate_specialized_function_value(&template)? else {
            return Ok(());
        };
        self.tracked_array_function_values
            .entry(name.to_string())
            .or_default()
            .insert(index, specialized);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_tracked_array_specialized_function_value(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<SpecializedFunctionValue> {
        let Expression::Identifier(name) = object else {
            return None;
        };
        let index = argument_index_from_expression(property)?;
        self.tracked_array_function_values
            .get(name)
            .and_then(|bindings| bindings.get(&index))
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn emit_specialized_callee_call(
        &mut self,
        callee: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if let Some(specialized) = self.resolve_specialized_function_value_from_expression(callee) {
            return self.emit_specialized_function_value_call(&specialized, arguments);
        }

        let Expression::Member { object, property } = callee else {
            return Ok(false);
        };
        let Some(specialized) =
            self.resolve_tracked_array_specialized_function_value(object, property)
        else {
            return Ok(false);
        };
        let Expression::Identifier(name) = object.as_ref() else {
            return Ok(false);
        };
        let Some(index) = argument_index_from_expression(property) else {
            return Ok(false);
        };
        if let Some(slot) = self.runtime_array_slot(name, index) {
            self.push_local_get(slot.present_local);
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.emit_specialized_function_value_call(&specialized, arguments)?;
            self.instructions.push(0x05);
            self.push_i32_const(JS_UNDEFINED_TAG);
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(true);
        }
        self.emit_specialized_function_value_call(&specialized, arguments)
    }
}
