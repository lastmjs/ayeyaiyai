use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_member_read_without_prelude(
        &mut self,
        object: &Expression,
        property: &Expression,
    ) -> DirectResult<()> {
        if matches!(property, Expression::String(property_name) if property_name == "global")
            && matches!(
                object,
                Expression::Call { callee, arguments }
                    if arguments.is_empty()
                        && matches!(
                            callee.as_ref(),
                            Expression::Member { object, property }
                                if matches!(object.as_ref(), Expression::Identifier(name) if name == "$262")
                                    && matches!(property.as_ref(), Expression::String(name) if name == "createRealm")
                        )
            )
        {
            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
            return Ok(());
        }
        if self.top_level_function && matches!(object, Expression::This) {
            let property = self
                .resolve_property_key_expression(property)
                .unwrap_or_else(|| self.materialize_static_expression(property));
            if let Expression::String(property_name) = property {
                if let Some(state) = self.module.global_property_descriptors.get(&property_name) {
                    if let Some(value) = state.writable.map(|_| state.value.clone()) {
                        self.emit_numeric_expression(&value)?;
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                    return Ok(());
                }
                if property_name == "NaN" {
                    self.push_i32_const(JS_NAN_TAG);
                    return Ok(());
                }
                if property_name == "undefined" {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
                if let Some(kind) = builtin_identifier_kind(&property_name) {
                    match kind {
                        StaticValueKind::Function => {
                            self.push_i32_const(
                                builtin_function_runtime_value(&property_name)
                                    .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                            );
                            return Ok(());
                        }
                        StaticValueKind::Object => {
                            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(text) = self.resolve_static_string_value(&Expression::Member {
            object: Box::new(self.materialize_static_expression(object)),
            property: Box::new(self.materialize_static_expression(property)),
        }) {
            self.emit_static_string_literal(&text)?;
            return Ok(());
        }
        if matches!(object, Expression::Identifier(name) if name == "Number" && self.is_unshadowed_builtin_identifier(name))
            && matches!(property, Expression::String(property_name) if property_name == "NaN")
        {
            self.push_i32_const(JS_NAN_TAG);
            return Ok(());
        }
        if let Some(step_binding) = self.resolve_iterator_step_binding_from_expression(object) {
            if let Expression::String(property_name) = property {
                match property_name.as_str() {
                    "done" => {
                        match step_binding {
                            IteratorStepBinding::Runtime { done_local, .. } => {
                                self.push_local_get(done_local);
                            }
                        }
                        return Ok(());
                    }
                    "value" => {
                        match step_binding {
                            IteratorStepBinding::Runtime { value_local, .. } => {
                                self.push_local_get(value_local);
                            }
                        }
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
        let static_array_property = if inline_summary_side_effect_free_expression(property)
            && !self.expression_depends_on_active_loop_assignment(property)
        {
            self.resolve_property_key_expression(property)
                .unwrap_or_else(|| self.materialize_static_expression(property))
        } else {
            property.clone()
        };
        if let Expression::Identifier(name) = object {
            if self.local_typed_array_view_bindings.contains_key(name) {
                if matches!(&static_array_property, Expression::String(text) if text == "length") {
                    if let Some(length_local) = self.runtime_array_length_locals.get(name).copied()
                    {
                        self.push_local_get(length_local);
                    } else {
                        self.push_i32_const(0);
                    }
                    return Ok(());
                }
                if let Some(index) = argument_index_from_expression(&static_array_property) {
                    if let Some(oob_local) = self.runtime_typed_array_oob_locals.get(name).copied()
                    {
                        self.push_local_get(oob_local);
                        self.instructions.push(0x04);
                        self.instructions.push(I32_TYPE);
                        self.push_control_frame();
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        self.instructions.push(0x05);
                        if !self.emit_runtime_array_slot_read(name, index)? {
                            self.push_i32_const(JS_UNDEFINED_TAG);
                        }
                        self.instructions.push(0x0b);
                        self.pop_control_frame();
                    } else if !self.emit_runtime_array_slot_read(name, index)? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                    return Ok(());
                }
            }
        }
        if let Some(bytes_per_element) =
            self.resolve_typed_array_builtin_bytes_per_element(object, property)
        {
            self.push_i32_const(bytes_per_element as i32);
            return Ok(());
        }
        if let Some(function_name) = self.resolve_function_name_value(object, property) {
            self.emit_static_string_literal(&function_name)?;
            return Ok(());
        }
        if let Some(function_length) = self.resolve_user_function_length(object, property) {
            self.push_i32_const(function_length as i32);
            return Ok(());
        }
        if let Some(function_binding) = self.resolve_member_function_binding(object, property) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.module.user_function_map.get(&function_name) {
                        self.push_i32_const(user_function_runtime_value(user_function));
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(_) => {
                    self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                }
            }
            return Ok(());
        }
        if let Some(function_binding) = self.resolve_member_getter_binding(object, property) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) =
                        self.module.user_function_map.get(&function_name).cloned()
                    {
                        self.emit_user_function_call_with_function_this_binding(
                            &user_function,
                            &[],
                            object,
                            None,
                        )?;
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    let callee = Expression::Identifier(function_name);
                    if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
            }
            return Ok(());
        }
        if matches!(property, Expression::String(property_name) if property_name == "caller") {
            if let Some(strict) = self.resolve_arguments_callee_strictness(object) {
                if strict {
                    return self.emit_error_throw();
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                return Ok(());
            }
        }
        if self.is_restricted_arrow_function_property(object, property) {
            self.emit_numeric_expression(object)?;
            self.instructions.push(0x1a);
            return self.emit_named_error_throw("TypeError");
        }
        if let Expression::Identifier(name) = object {
            if let Some(descriptor) = self.local_descriptor_bindings.get(name) {
                if let Expression::String(property_name) = property {
                    match property_name.as_str() {
                        "value" => {
                            if let Some(value) = descriptor.value.clone() {
                                self.emit_numeric_expression(&value)?;
                            } else {
                                self.push_i32_const(JS_UNDEFINED_TAG);
                            }
                            return Ok(());
                        }
                        "configurable" => {
                            self.push_i32_const(if descriptor.configurable { 1 } else { 0 });
                            return Ok(());
                        }
                        "enumerable" => {
                            self.push_i32_const(if descriptor.enumerable { 1 } else { 0 });
                            return Ok(());
                        }
                        "writable" => {
                            if let Some(writable) = descriptor.writable {
                                self.push_i32_const(if writable { 1 } else { 0 });
                            } else {
                                self.push_i32_const(JS_UNDEFINED_TAG);
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            }
            if let Some(index) = argument_index_from_expression(&static_array_property) {
                if self.emit_global_runtime_array_slot_read(name, index)? {
                    return Ok(());
                }
                if self.emit_runtime_array_slot_read(name, index)? {
                    return Ok(());
                }
            }
        }
        if let Some(array_binding) = self.resolve_array_binding_from_expression(object) {
            if matches!(&static_array_property, Expression::String(text) if text == "length") {
                if let Expression::Identifier(name) = object
                    && self.emit_global_runtime_array_length_read(name)
                {
                    return Ok(());
                }
                if let Some(length_local) = self.runtime_array_length_local_for_expression(object) {
                    self.push_local_get(length_local);
                } else {
                    self.push_i32_const(array_binding.values.len() as i32);
                }
                return Ok(());
            }
            if let Some(index) = argument_index_from_expression(&static_array_property) {
                if let Expression::Identifier(name) = object
                    && self.emit_global_runtime_array_slot_read(name, index)?
                {
                    return Ok(());
                }
                if let Some(Some(value)) = array_binding.values.get(index as usize) {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
        }
        if let Some(binding) = self.resolve_runtime_object_property_shadow_binding(object, property)
        {
            let fallback_value = self
                .resolve_object_binding_from_expression(object)
                .and_then(|object_binding| {
                    self.resolve_object_binding_property_value(&object_binding, property)
                });
            self.push_global_get(binding.present_index);
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            self.push_global_get(binding.value_index);
            self.instructions.push(0x05);
            if let Some(fallback_value) = fallback_value {
                self.emit_numeric_expression(&fallback_value)?;
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            self.instructions.push(0x0b);
            self.pop_control_frame();
            return Ok(());
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(object) {
            if let Some(value) =
                self.resolve_object_binding_property_value(&object_binding, property)
            {
                self.emit_numeric_expression(&value)?;
            } else if matches!(property, Expression::String(text) if text == "constructor") {
                if let Some(binding) = self.resolve_constructed_object_constructor_binding(object) {
                    match binding {
                        LocalFunctionBinding::User(function_name) => {
                            if let Some(user_function) =
                                self.module.user_function_map.get(&function_name)
                            {
                                self.push_i32_const(user_function_runtime_value(user_function));
                            } else {
                                self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                            }
                        }
                        LocalFunctionBinding::Builtin(function_name) => {
                            self.push_i32_const(
                                builtin_function_runtime_value(&function_name)
                                    .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                            );
                        }
                    }
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
            } else {
                self.push_i32_const(JS_UNDEFINED_TAG);
            }
            return Ok(());
        }
        if let Expression::String(text) = object {
            if let Some(index) = argument_index_from_expression(property) {
                if let Some(character) = text.chars().nth(index as usize) {
                    self.emit_numeric_expression(&Expression::String(character.to_string()))?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
            if matches!(property, Expression::String(name) if name == "length") {
                self.push_i32_const(text.chars().count() as i32);
                return Ok(());
            }
        }
        if let Some(arguments_binding) = self.resolve_arguments_binding_from_expression(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                if !arguments_binding.length_present {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                } else {
                    self.emit_numeric_expression(&arguments_binding.length_value)?;
                }
                return Ok(());
            }
            if matches!(property, Expression::String(property_name) if property_name == "callee") {
                if arguments_binding.strict {
                    return self.emit_error_throw();
                }
                if !arguments_binding.callee_present {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                } else if let Some(value) = arguments_binding.callee_value.as_ref() {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
            if let Some(index) = argument_index_from_expression(property) {
                if let Some(value) = arguments_binding.values.get(index as usize) {
                    self.emit_numeric_expression(value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                return Ok(());
            }
            return self.emit_dynamic_arguments_binding_property_read(&arguments_binding, property);
        }
        if self.is_direct_arguments_object(object) {
            if matches!(property, Expression::String(text) if text == "length") {
                return self.emit_direct_arguments_length();
            }
            if matches!(property, Expression::String(text) if text == "callee") {
                return self.emit_direct_arguments_callee();
            }
            if let Some(index) = argument_index_from_expression(property) {
                return self.emit_arguments_slot_read(index);
            }
            return self.emit_dynamic_direct_arguments_property_read(property);
        }
        if let Some(returned_value) =
            self.resolve_returned_member_value_from_expression(object, property)
        {
            self.emit_numeric_expression(&returned_value)?;
            return Ok(());
        }
        if self.emit_runtime_user_function_property_read(object, property)? {
            return Ok(());
        }
        if matches!(property, Expression::String(text) if text == "constructor") {
            if let Some(binding) = self.resolve_constructed_object_constructor_binding(object) {
                match binding {
                    LocalFunctionBinding::User(function_name) => {
                        if let Some(user_function) =
                            self.module.user_function_map.get(&function_name)
                        {
                            self.push_i32_const(user_function_runtime_value(user_function));
                        } else {
                            self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                        }
                    }
                    LocalFunctionBinding::Builtin(function_name) => {
                        self.push_i32_const(
                            builtin_function_runtime_value(&function_name)
                                .unwrap_or(JS_TYPEOF_FUNCTION_TAG),
                        );
                    }
                }
            } else {
                self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
            }
            return Ok(());
        }
        if self.resolve_array_binding_from_expression(object).is_some() {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        }
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_scoped_property_store_from_local(
        &mut self,
        scope_object: &Expression,
        name: &str,
        value_local: u32,
        value_expression: &Expression,
    ) -> DirectResult<()> {
        let property = Expression::String(name.to_string());
        if let Some(binding) =
            self.resolve_runtime_object_property_shadow_binding(scope_object, &property)
        {
            self.push_local_get(value_local);
            self.push_global_set(binding.value_index);
            self.push_i32_const(1);
            self.push_global_set(binding.present_index);
        }
        if let Some(setter_binding) = self.resolve_member_setter_binding(scope_object, &property) {
            let receiver_hidden_name = self.allocate_named_hidden_local(
                "scoped_setter_receiver",
                self.infer_value_kind(scope_object)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let receiver_local = self
                .locals
                .get(&receiver_hidden_name)
                .copied()
                .expect("fresh scoped setter receiver hidden local must exist");
            self.emit_numeric_expression(scope_object)?;
            self.push_local_set(receiver_local);
            let receiver_expression = Expression::Identifier(receiver_hidden_name);
            if self.emit_function_binding_call_with_function_this_binding_from_argument_locals(
                &setter_binding,
                &[value_local],
                1,
                &receiver_expression,
            )? {
                self.instructions.push(0x1a);
            }
            self.push_local_get(value_local);
            return Ok(());
        }

        let materialized_value = self.materialize_static_expression(value_expression);
        if let Expression::Identifier(scope_name) = scope_object {
            if let Some(object_binding) = self.local_object_bindings.get_mut(scope_name) {
                object_binding_set_property(
                    object_binding,
                    property.clone(),
                    materialized_value.clone(),
                );
                self.push_local_get(value_local);
                return Ok(());
            }
            if let Some(object_binding) = self.module.global_object_bindings.get_mut(scope_name) {
                object_binding_set_property(object_binding, property, materialized_value);
                self.push_local_get(value_local);
                return Ok(());
            }
            if let Some(hidden_name) = self.resolve_user_function_capture_hidden_name(scope_name) {
                if let Some(object_binding) =
                    self.module.global_object_bindings.get_mut(&hidden_name)
                {
                    object_binding_set_property(object_binding, property, materialized_value);
                    self.push_local_get(value_local);
                    return Ok(());
                }
            }
        }

        self.push_local_get(value_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_scoped_property_update(
        &mut self,
        scope_object: &Expression,
        name: &str,
        op: UpdateOp,
        prefix: bool,
    ) -> DirectResult<()> {
        let opcode = match op {
            UpdateOp::Increment => 0x6a,
            UpdateOp::Decrement => 0x6b,
        };
        let property = Expression::String(name.to_string());
        let member_expression = Expression::Member {
            object: Box::new(scope_object.clone()),
            property: Box::new(property.clone()),
        };
        let previous_kind = self
            .infer_value_kind(&member_expression)
            .unwrap_or(StaticValueKind::Unknown);
        let current_value = self
            .resolve_object_binding_from_expression(scope_object)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &property).cloned()
            })
            .unwrap_or(Expression::Undefined);
        let increment = match op {
            UpdateOp::Increment => 1.0,
            UpdateOp::Decrement => -1.0,
        };

        match previous_kind {
            StaticValueKind::Undefined
            | StaticValueKind::String
            | StaticValueKind::Object
            | StaticValueKind::Function
            | StaticValueKind::Symbol
            | StaticValueKind::BigInt => {
                let nan_local = self.allocate_temp_local();
                self.push_i32_const(JS_NAN_TAG);
                self.push_local_set(nan_local);
                self.emit_scoped_property_store_from_local(
                    scope_object,
                    name,
                    nan_local,
                    &Expression::Number(f64::NAN),
                )?;
                self.instructions.push(0x1a);
                self.push_local_get(nan_local);
                return Ok(());
            }
            StaticValueKind::Null => {
                let previous_local = self.allocate_temp_local();
                let next_local = self.allocate_temp_local();
                self.push_i32_const(0);
                self.push_local_set(previous_local);
                self.push_i32_const(increment as i32);
                self.push_local_set(next_local);
                self.emit_scoped_property_store_from_local(
                    scope_object,
                    name,
                    next_local,
                    &Expression::Number(increment),
                )?;
                self.instructions.push(0x1a);
                if prefix {
                    self.push_local_get(next_local);
                } else {
                    self.push_local_get(previous_local);
                }
                return Ok(());
            }
            _ => {}
        }

        let previous_local = self.allocate_temp_local();
        let next_local = self.allocate_temp_local();
        self.emit_scoped_property_read(scope_object, name)?;
        self.push_local_set(previous_local);
        self.push_local_get(previous_local);
        self.push_i32_const(1);
        self.instructions.push(opcode);
        self.push_local_set(next_local);
        let next_expression = match previous_kind {
            StaticValueKind::Bool => {
                let previous = match self.materialize_static_expression(&current_value) {
                    Expression::Bool(value) => {
                        if value {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    _ => 0.0,
                };
                Expression::Number(previous + increment)
            }
            _ => self
                .resolve_static_number_value(&current_value)
                .map(|value| Expression::Number(value + increment))
                .unwrap_or(Expression::Number(f64::NAN)),
        };
        self.emit_scoped_property_store_from_local(
            scope_object,
            name,
            next_local,
            &next_expression,
        )?;
        self.instructions.push(0x1a);
        if prefix {
            self.push_local_get(next_local);
        } else {
            self.push_local_get(previous_local);
        }
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn resolve_home_object_name_for_function(
        &self,
        function_name: &str,
    ) -> Option<String> {
        if let Some(home_object_name) = self
            .module
            .user_function_map
            .get(function_name)?
            .home_object_binding
            .as_ref()
        {
            return Some(home_object_name.clone());
        }
        self.module.global_value_bindings.iter().find_map(|(name, value)| {
            let Expression::Object(entries) = value else {
                return None;
            };
            entries.iter().find_map(|entry| {
                let candidate = match entry {
                    crate::ir::hir::ObjectEntry::Data { value, .. } => value,
                    crate::ir::hir::ObjectEntry::Getter { getter, .. } => getter,
                    crate::ir::hir::ObjectEntry::Setter { setter, .. } => setter,
                    crate::ir::hir::ObjectEntry::Spread(_) => return None,
                };
                matches!(candidate, Expression::Identifier(candidate_name) if candidate_name == function_name)
                    .then_some(name.clone())
            })
        })
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_base_expression_with_context(
        &self,
        current_function_name: Option<&str>,
    ) -> Option<Expression> {
        let function_name = current_function_name?;
        let home_object_name = self.resolve_home_object_name_for_function(function_name)?;
        self.module
            .global_object_prototype_bindings
            .get(&home_object_name)
            .cloned()
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_runtime_prototype_binding_with_context(
        &self,
        current_function_name: Option<&str>,
    ) -> Option<(String, GlobalObjectRuntimePrototypeBinding)> {
        let function_name = current_function_name?;
        let home_object_name = self.resolve_home_object_name_for_function(function_name)?;
        let binding = self
            .module
            .global_runtime_prototype_bindings
            .get(&home_object_name)?
            .clone();
        Some((home_object_name, binding))
    }

    pub(in crate::backend::direct_wasm) fn emit_runtime_super_property_value_from_base(
        &mut self,
        base: Option<&Expression>,
        property: &Expression,
    ) -> DirectResult<()> {
        let Some(base) = base else {
            self.push_i32_const(JS_UNDEFINED_TAG);
            return Ok(());
        };
        if let Some(function_binding) = self.resolve_member_function_binding(base, property) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) = self.module.user_function_map.get(&function_name) {
                        self.push_i32_const(user_function_runtime_value(user_function));
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(_) => {
                    self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                }
            }
            return Ok(());
        }
        if let Some(function_binding) = self.resolve_member_getter_binding(base, property) {
            match function_binding {
                LocalFunctionBinding::User(function_name) => {
                    if let Some(user_function) =
                        self.module.user_function_map.get(&function_name).cloned()
                    {
                        self.emit_user_function_call_with_function_this_binding(
                            &user_function,
                            &[],
                            base,
                            None,
                        )?;
                    } else {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
                LocalFunctionBinding::Builtin(function_name) => {
                    let callee = Expression::Identifier(function_name);
                    if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                }
            }
            return Ok(());
        }
        let materialized_property = self.materialize_static_expression(property);
        if let Some(object_binding) = self.resolve_object_binding_from_expression(base)
            && let Some(value) =
                object_binding_lookup_value(&object_binding, &materialized_property).cloned()
        {
            self.emit_numeric_expression(&value)?;
            return Ok(());
        }
        self.push_i32_const(JS_UNDEFINED_TAG);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_super_member_read_via_runtime_prototype_binding(
        &mut self,
        property: &Expression,
    ) -> DirectResult<bool> {
        let Some((_, binding)) = self.resolve_super_runtime_prototype_binding_with_context(
            self.current_user_function_name.as_deref(),
        ) else {
            return Ok(false);
        };
        let Some(global_index) = binding.global_index else {
            return Ok(false);
        };
        let resolved_property = self
            .resolve_property_key_expression(property)
            .unwrap_or_else(|| self.materialize_static_expression(property));
        if !matches!(
            resolved_property,
            Expression::String(_) | Expression::Identifier(_) | Expression::Member { .. }
        ) {
            return Ok(false);
        }

        let state_local = self.allocate_temp_local();
        self.push_global_get(global_index);
        self.push_local_set(state_local);

        let mut open_frames = 0;
        for (variant_index, prototype) in binding.variants.iter().enumerate() {
            self.push_local_get(state_local);
            self.push_i32_const(variant_index as i32);
            self.push_binary_op(BinaryOp::Equal)?;
            self.instructions.push(0x04);
            self.instructions.push(I32_TYPE);
            self.push_control_frame();
            open_frames += 1;
            self.emit_runtime_super_property_value_from_base(
                prototype.as_ref(),
                &resolved_property,
            )?;
            self.instructions.push(0x05);
        }

        self.push_i32_const(JS_UNDEFINED_TAG);
        for _ in 0..open_frames {
            self.instructions.push(0x0b);
            self.pop_control_frame();
        }
        Ok(true)
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_function_binding(
        &self,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_super_function_binding_with_context(
            property,
            self.current_user_function_name.as_deref(),
        )
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_function_binding_with_context(
        &self,
        property: &Expression,
        current_function_name: Option<&str>,
    ) -> Option<LocalFunctionBinding> {
        let base = self.resolve_super_base_expression_with_context(current_function_name)?;
        let materialized_property = self.materialize_static_expression(property);
        self.resolve_member_function_binding(&base, property)
            .or_else(|| {
                self.resolve_object_binding_from_expression(&base)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(&object_binding, &materialized_property)
                            .cloned()
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(&value))
            })
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_getter_binding(
        &self,
        property: &Expression,
    ) -> Option<LocalFunctionBinding> {
        let base = self.resolve_super_base_expression_with_context(
            self.current_user_function_name.as_deref(),
        )?;
        self.resolve_member_getter_binding(&base, property)
    }

    pub(in crate::backend::direct_wasm) fn resolve_super_value_expression(
        &self,
        property: &Expression,
    ) -> Option<Expression> {
        let base = self.resolve_super_base_expression_with_context(
            self.current_user_function_name.as_deref(),
        )?;
        let materialized_property = self.materialize_static_expression(property);
        self.resolve_object_binding_from_expression(&base)
            .and_then(|object_binding| {
                object_binding_lookup_value(&object_binding, &materialized_property).cloned()
            })
    }

    pub(in crate::backend::direct_wasm) fn binding_name_is_global(&self, name: &str) -> bool {
        self.top_level_function
            && self.module.global_bindings.contains_key(name)
            && !self.locals.contains_key(name)
    }

    pub(in crate::backend::direct_wasm) fn binding_key_is_global(
        &self,
        key: &MemberFunctionBindingKey,
    ) -> bool {
        match &key.target {
            MemberFunctionBindingTarget::Identifier(name)
            | MemberFunctionBindingTarget::Prototype(name) => self.binding_name_is_global(name),
        }
    }

    pub(in crate::backend::direct_wasm) fn resolve_named_function_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
        descriptor_name: &str,
    ) -> Option<LocalFunctionBinding> {
        let Expression::Object(entries) = descriptor else {
            return None;
        };
        for entry in entries {
            let crate::ir::hir::ObjectEntry::Data { key, value } = entry else {
                continue;
            };
            if matches!(key, Expression::String(name) if name == descriptor_name) {
                return self.resolve_function_binding_from_expression(value);
            }
        }
        None
    }

    pub(in crate::backend::direct_wasm) fn resolve_function_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_named_function_binding_from_descriptor_expression(descriptor, "value")
    }

    pub(in crate::backend::direct_wasm) fn resolve_getter_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_named_function_binding_from_descriptor_expression(descriptor, "get")
    }

    pub(in crate::backend::direct_wasm) fn resolve_setter_binding_from_descriptor_expression(
        &self,
        descriptor: &Expression,
    ) -> Option<LocalFunctionBinding> {
        self.resolve_named_function_binding_from_descriptor_expression(descriptor, "set")
    }
}
