use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_numeric_expression(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        match expression {
            Expression::Number(value) => {
                if value.is_nan() {
                    self.push_i32_const(JS_NAN_TAG);
                } else {
                    self.push_i32_const(f64_to_i32(*value)?);
                }
                Ok(())
            }
            Expression::BigInt(value) => {
                self.push_i32_const(parse_bigint_to_i32(value)?);
                Ok(())
            }
            Expression::String(text) => {
                match parse_string_to_i32(text) {
                    Ok(parsed) => self.push_i32_const(parsed),
                    Err(Unsupported("string literal collides with reserved JS tag")) => {
                        return Err(Unsupported("string literal collides with reserved JS tag"));
                    }
                    Err(_) => {
                        self.emit_static_string_literal(text)?;
                    }
                }
                Ok(())
            }
            Expression::Null => {
                self.push_i32_const(JS_NULL_TAG);
                Ok(())
            }
            Expression::Undefined => {
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            Expression::Bool(value) => {
                self.push_i32_const(if *value { 1 } else { 0 });
                Ok(())
            }
            Expression::Identifier(name) => {
                if let Some(scope_object) = self.resolve_with_scope_binding(name)? {
                    self.emit_scoped_property_read(&scope_object, name)?;
                } else {
                    self.emit_plain_identifier_read(name)?;
                }
                Ok(())
            }
            Expression::Array(elements) => {
                for element in elements {
                    match element {
                        crate::ir::hir::ArrayElement::Expression(expression)
                        | crate::ir::hir::ArrayElement::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                Ok(())
            }
            Expression::Object(entries) => {
                for entry in entries {
                    match entry {
                        crate::ir::hir::ObjectEntry::Data { key, value } => {
                            self.emit_property_key_expression_effects(key)?;
                            self.emit_numeric_expression(value)?;
                            self.instructions.push(0x1a);
                        }
                        crate::ir::hir::ObjectEntry::Getter { key, getter } => {
                            self.emit_property_key_expression_effects(key)?;
                            self.emit_numeric_expression(getter)?;
                            self.instructions.push(0x1a);
                        }
                        crate::ir::hir::ObjectEntry::Setter { key, setter } => {
                            self.emit_property_key_expression_effects(key)?;
                            self.emit_numeric_expression(setter)?;
                            self.instructions.push(0x1a);
                        }
                        crate::ir::hir::ObjectEntry::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                            self.emit_object_spread_copy_data_properties_effects(expression)?;
                        }
                    }
                }
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                Ok(())
            }
            Expression::Assign { name, value } => {
                let scoped_target = self.resolve_with_scope_binding(name)?;
                self.emit_numeric_expression(value)?;
                if let Some(scope_object) = scoped_target {
                    let value_local = self.allocate_temp_local();
                    self.push_local_set(value_local);
                    self.emit_scoped_property_store_from_local(
                        &scope_object,
                        name,
                        value_local,
                        value,
                    )?;
                } else {
                    let value_local = self.allocate_temp_local();
                    self.push_local_set(value_local);
                    self.emit_store_identifier_value_local(name, value, value_local)?;
                    self.push_local_get(value_local);
                }
                Ok(())
            }
            Expression::Unary { op, expression } => match op {
                UnaryOp::TypeOf => {
                    if let Expression::Identifier(name) = expression.as_ref()
                        && self.eval_lexical_initialized_locals.contains_key(name)
                    {
                        self.emit_eval_lexical_binding_read(name)?;
                        self.instructions.push(0x1a);
                    }
                    if let Expression::Identifier(name) = expression.as_ref()
                        && self.resolve_current_local_binding(name).is_none()
                        && !self.module.global_bindings.contains_key(name)
                        && self.emit_typeof_user_function_capture_binding(name)?
                    {
                        return Ok(());
                    }
                    if let Expression::Identifier(name) = expression.as_ref()
                        && self.resolve_current_local_binding(name).is_none()
                        && !self.module.global_bindings.contains_key(name)
                        && self.emit_typeof_eval_local_function_binding(name)?
                    {
                        return Ok(());
                    }
                    if self
                        .resolve_function_binding_from_expression(expression)
                        .is_some()
                    {
                        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                        return Ok(());
                    }
                    if let Some(strict) = self.resolve_arguments_callee_strictness(expression) {
                        if strict {
                            return self.emit_error_throw();
                        }
                        self.push_i32_const(JS_TYPEOF_FUNCTION_TAG);
                        return Ok(());
                    }
                    if let Expression::Identifier(name) = expression.as_ref()
                        && self.is_identifier_bound(name)
                    {
                        self.emit_runtime_typeof_tag(expression)?;
                        return Ok(());
                    }
                    let Some(type_tag) = self
                        .infer_typeof_operand_kind(expression)
                        .and_then(StaticValueKind::as_typeof_tag)
                    else {
                        self.emit_runtime_typeof_tag(expression)?;
                        return Ok(());
                    };
                    self.push_i32_const(type_tag);
                    Ok(())
                }
                UnaryOp::Not => {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x45);
                    Ok(())
                }
                UnaryOp::BitwiseNot => {
                    self.emit_numeric_expression(expression)?;
                    self.push_i32_const(-1);
                    self.instructions.push(0x73);
                    Ok(())
                }
                UnaryOp::Negate => {
                    match expression.as_ref() {
                        Expression::Number(value) if value.is_finite() && value.fract() == 0.0 => {
                            let integer = -(*value as i64);
                            if is_reserved_js_runtime_value(integer) {
                                return Err(Unsupported(
                                    "number literal collides with reserved JS tag",
                                ));
                            }
                        }
                        Expression::BigInt(value) => {
                            let integer = format!("-{}", value.strip_suffix('n').unwrap_or(value));
                            if let Ok(parsed) = integer.parse::<i64>()
                                && is_reserved_js_runtime_value(parsed)
                            {
                                return Err(Unsupported(
                                    "bigint literal collides with reserved JS tag",
                                ));
                            }
                        }
                        _ => {}
                    }
                    self.push_i32_const(0);
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x6b);
                    Ok(())
                }
                UnaryOp::Plus => self.emit_numeric_expression(expression),
                UnaryOp::Void => {
                    let temp_local = self.allocate_temp_local();
                    self.emit_numeric_expression(expression)?;
                    self.push_local_set(temp_local);
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    Ok(())
                }
                UnaryOp::Delete => {
                    match expression.as_ref() {
                        Expression::Identifier(name)
                            if self.resolve_current_local_binding(name).is_none()
                                && !self.module.global_bindings.contains_key(name)
                                && self.resolve_eval_local_function_hidden_name(name).is_some() =>
                        {
                            self.clear_eval_local_function_binding_metadata(name);
                            self.emit_delete_eval_local_function_binding(name)?;
                            return Ok(());
                        }
                        Expression::Identifier(name)
                            if self.resolve_current_local_binding(name).is_none()
                                && !self.module.global_bindings.contains_key(name)
                                && self.module.implicit_global_bindings.contains_key(name) =>
                        {
                            self.deleted_builtin_identifiers.remove(name);
                            self.emit_delete_implicit_global_binding(name)?;
                            return Ok(());
                        }
                        Expression::Identifier(name)
                            if self.resolve_current_local_binding(name).is_none()
                                && !self.module.global_bindings.contains_key(name)
                                && self.is_unshadowed_builtin_identifier(name)
                                && builtin_identifier_delete_returns_true(name) =>
                        {
                            self.clear_static_identifier_binding_metadata(name);
                            self.deleted_builtin_identifiers.insert(name.clone());
                            self.push_i32_const(1);
                            return Ok(());
                        }
                        Expression::Identifier(name) if self.is_identifier_bound(name) => {
                            self.push_i32_const(0);
                        }
                        Expression::Identifier(_) => {
                            self.push_i32_const(1);
                        }
                        Expression::Member { object, property } if matches!(property.as_ref(), Expression::String(property_name) if property_name == "callee" || property_name == "length") =>
                        {
                            let Expression::String(property_name) = property.as_ref() else {
                                unreachable!("filtered above");
                            };
                            if self.is_direct_arguments_object(object) {
                                match property_name.as_str() {
                                    "callee" => {
                                        if self.strict_mode {
                                            self.push_i32_const(0);
                                        } else {
                                            self.apply_current_arguments_effect(
                                                "callee",
                                                ArgumentsPropertyEffect::Delete,
                                            );
                                            self.push_i32_const(1);
                                        }
                                    }
                                    "length" => {
                                        self.apply_current_arguments_effect(
                                            "length",
                                            ArgumentsPropertyEffect::Delete,
                                        );
                                        self.push_i32_const(1);
                                    }
                                    _ => unreachable!("filtered above"),
                                }
                                self.emit_delete_result_or_throw_if_strict()?;
                                return Ok(());
                            }
                            if let Some(arguments_binding) =
                                self.resolve_arguments_binding_from_expression(object)
                            {
                                self.emit_numeric_expression(object)?;
                                self.instructions.push(0x1a);
                                self.emit_numeric_expression(property)?;
                                self.instructions.push(0x1a);
                                if property_name == "callee" && arguments_binding.strict {
                                    self.push_i32_const(0);
                                } else {
                                    self.update_named_arguments_binding_effect(
                                        object,
                                        property_name,
                                        ArgumentsPropertyEffect::Delete,
                                    );
                                    self.push_i32_const(1);
                                }
                                return Ok(());
                            }
                            if property_name == "length"
                                && self.resolve_array_binding_from_expression(object).is_some()
                            {
                                self.push_i32_const(0);
                                return Ok(());
                            }
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                            self.push_i32_const(1);
                        }
                        Expression::Member { object, property }
                            if self.is_direct_arguments_object(object)
                                && argument_index_from_expression(property).is_some() =>
                        {
                            self.emit_arguments_slot_delete(
                                argument_index_from_expression(property).expect("checked above"),
                            );
                            self.emit_delete_result_or_throw_if_strict()?;
                            return Ok(());
                        }
                        Expression::Member { object, property }
                            if argument_index_from_expression(property).is_some() =>
                        {
                            let index =
                                argument_index_from_expression(property).expect("checked above");
                            if let Expression::Identifier(name) = object.as_ref() {
                                if let Some(array_binding) = self.local_array_bindings.get_mut(name)
                                {
                                    if let Some(value) =
                                        array_binding.values.get_mut(index as usize)
                                    {
                                        *value = None;
                                    }
                                    self.clear_runtime_array_slot(name, index);
                                    self.push_i32_const(1);
                                    return Ok(());
                                }
                                if let Some(array_binding) =
                                    self.module.global_array_bindings.get_mut(name)
                                {
                                    if let Some(value) =
                                        array_binding.values.get_mut(index as usize)
                                    {
                                        *value = None;
                                    }
                                    self.clear_global_runtime_array_slot(name, index);
                                    self.push_i32_const(1);
                                    return Ok(());
                                }
                                if let Some(arguments_binding) =
                                    self.local_arguments_bindings.get_mut(name)
                                {
                                    if let Some(value) =
                                        arguments_binding.values.get_mut(index as usize)
                                    {
                                        *value = Expression::Undefined;
                                    }
                                    self.push_i32_const(1);
                                    return Ok(());
                                }
                                if let Some(arguments_binding) =
                                    self.module.global_arguments_bindings.get_mut(name)
                                {
                                    if let Some(value) =
                                        arguments_binding.values.get_mut(index as usize)
                                    {
                                        *value = Expression::Undefined;
                                    }
                                    self.push_i32_const(1);
                                    return Ok(());
                                }
                            }
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                            self.push_i32_const(1);
                        }
                        Expression::Member { object, property } => {
                            let resolved_property = self
                                .resolve_property_key_expression(property)
                                .unwrap_or_else(|| self.materialize_static_expression(property));
                            if matches!(
                                resolved_property,
                                Expression::String(ref property_name) if property_name == "length"
                            ) && self.resolve_array_binding_from_expression(object).is_some()
                            {
                                self.push_i32_const(0);
                                return Ok(());
                            }
                            if let (
                                Expression::Identifier(object_name),
                                Expression::String(property_name),
                            ) = (
                                self.materialize_static_expression(object),
                                resolved_property.clone(),
                            ) && self.is_unshadowed_builtin_identifier(&object_name)
                                && builtin_member_delete_returns_false(&object_name, &property_name)
                            {
                                self.push_i32_const(0);
                                return Ok(());
                            }
                            if let Expression::Identifier(name) = object.as_ref() {
                                let materialized_property = resolved_property;
                                self.clear_runtime_object_property_shadow_binding(
                                    object,
                                    &materialized_property,
                                );
                                if let Some(object_binding) =
                                    self.local_object_bindings.get_mut(name)
                                {
                                    object_binding_remove_property(
                                        object_binding,
                                        &materialized_property,
                                    );
                                    self.push_i32_const(1);
                                    return Ok(());
                                }
                                if let Some(object_binding) =
                                    self.module.global_object_bindings.get_mut(name)
                                {
                                    object_binding_remove_property(
                                        object_binding,
                                        &materialized_property,
                                    );
                                    self.push_i32_const(1);
                                    return Ok(());
                                }
                            }
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                            self.push_i32_const(1);
                        }
                        Expression::SuperMember { .. }
                        | Expression::AssignMember { .. }
                        | Expression::AssignSuperMember { .. }
                        | Expression::This => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                            self.push_i32_const(1);
                        }
                        _ => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                            self.push_i32_const(1);
                        }
                    }
                    Ok(())
                }
            },
            Expression::Member { object, property } => {
                if self.emit_direct_iterator_step_member_read(object, property)? {
                    return Ok(());
                }
                self.emit_numeric_expression(object)?;
                self.instructions.push(0x1a);
                let resolved_property = self.emit_property_key_expression_effects(property)?;
                let effective_property = resolved_property.as_ref().unwrap_or(property.as_ref());
                self.emit_member_read_without_prelude(object, effective_property)
            }
            Expression::Sent => {
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            Expression::NewTarget => {
                self.push_global_get(CURRENT_NEW_TARGET_GLOBAL_INDEX);
                Ok(())
            }
            Expression::SuperMember { property } => {
                if self.emit_super_member_read_via_runtime_prototype_binding(property)? {
                    return Ok(());
                }
                if let Some(function_binding) = self.resolve_super_function_binding(property) {
                    match function_binding {
                        LocalFunctionBinding::User(function_name) => {
                            if let Some(user_function) =
                                self.module.user_function_map.get(&function_name)
                            {
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
                if let Some(function_binding) = self.resolve_super_getter_binding(property) {
                    self.emit_numeric_expression(property)?;
                    self.instructions.push(0x1a);
                    let callee = match function_binding {
                        LocalFunctionBinding::User(function_name)
                        | LocalFunctionBinding::Builtin(function_name) => {
                            Expression::Identifier(function_name)
                        }
                    };
                    if !self.emit_arguments_slot_accessor_call(&callee, &[], 0, Some(&[]))? {
                        self.push_i32_const(JS_UNDEFINED_TAG);
                    }
                    return Ok(());
                }
                if let Some(value) = self.resolve_super_value_expression(property) {
                    self.emit_numeric_expression(&value)?;
                } else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                }
                Ok(())
            }
            Expression::AssignMember {
                object,
                property,
                value,
            } => {
                if matches!(property.as_ref(), Expression::String(property_name) if property_name == "callee" || property_name == "length")
                {
                    let Expression::String(property_name) = property.as_ref() else {
                        unreachable!("filtered above");
                    };
                    if self.is_direct_arguments_object(object) {
                        let temp_local = self.allocate_temp_local();
                        self.emit_numeric_expression(value)?;
                        self.push_local_set(temp_local);
                        if property_name == "callee" && self.strict_mode {
                            self.push_local_get(temp_local);
                            self.instructions.push(0x1a);
                            return self.emit_error_throw();
                        }
                        self.apply_current_arguments_effect(
                            property_name,
                            ArgumentsPropertyEffect::Assign((**value).clone()),
                        );
                        self.push_local_get(temp_local);
                        return Ok(());
                    }
                    if let Some(arguments_binding) =
                        self.resolve_arguments_binding_from_expression(object)
                    {
                        self.emit_numeric_expression(object)?;
                        self.instructions.push(0x1a);
                        self.emit_numeric_expression(property)?;
                        self.instructions.push(0x1a);
                        let temp_local = self.allocate_temp_local();
                        self.emit_numeric_expression(value)?;
                        self.push_local_set(temp_local);
                        if property_name == "callee" && arguments_binding.strict {
                            self.push_local_get(temp_local);
                            self.instructions.push(0x1a);
                            return self.emit_error_throw();
                        }
                        self.update_named_arguments_binding_effect(
                            object,
                            property_name,
                            ArgumentsPropertyEffect::Assign((**value).clone()),
                        );
                        self.push_local_get(temp_local);
                        return Ok(());
                    }
                }
                if self.is_direct_arguments_object(object) {
                    if let Some(index) = argument_index_from_expression(property) {
                        return self.emit_arguments_slot_write(index, value);
                    }
                    return self.emit_dynamic_direct_arguments_property_write(property, value);
                }
                if self.is_restricted_arrow_function_property(object, property) {
                    self.emit_numeric_expression(object)?;
                    self.instructions.push(0x1a);
                    self.emit_numeric_expression(property)?;
                    self.instructions.push(0x1a);
                    self.emit_numeric_expression(value)?;
                    self.instructions.push(0x1a);
                    return self.emit_named_error_throw("TypeError");
                }
                if let Expression::Identifier(name) = object.as_ref() {
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "prototype")
                    {
                        self.update_prototype_object_binding(name, value);
                    }
                }
                if let Some(function_binding) = self.resolve_member_setter_binding(object, property)
                {
                    let receiver_hidden_name = self.allocate_named_hidden_local(
                        "setter_receiver",
                        self.infer_value_kind(object)
                            .unwrap_or(StaticValueKind::Unknown),
                    );
                    let receiver_local = self
                        .locals
                        .get(&receiver_hidden_name)
                        .copied()
                        .expect("fresh setter receiver hidden local must exist");
                    let value_local = self.allocate_temp_local();
                    self.emit_numeric_expression(object)?;
                    self.push_local_set(receiver_local);
                    self.emit_numeric_expression(value)?;
                    self.push_local_set(value_local);
                    let receiver_expression = Expression::Identifier(receiver_hidden_name);
                    if self
                        .emit_function_binding_call_with_function_this_binding_from_argument_locals(
                            &function_binding,
                            &[value_local],
                            1,
                            &receiver_expression,
                        )?
                    {
                        self.instructions.push(0x1a);
                    }
                    self.push_local_get(value_local);
                    return Ok(());
                }
                if let Expression::Member {
                    object: prototype_object,
                    property: target_property,
                } = object.as_ref()
                    && matches!(target_property.as_ref(), Expression::String(name) if name == "prototype")
                {
                    let Expression::Identifier(name) = prototype_object.as_ref() else {
                        unreachable!("filtered above");
                    };
                    let materialized_property = self.materialize_static_expression(property);
                    let materialized = self.materialize_static_expression(value);
                    if let Some(object_binding) = self.local_prototype_object_bindings.get_mut(name)
                    {
                        object_binding_set_property(
                            object_binding,
                            materialized_property.clone(),
                            materialized.clone(),
                        );
                    }
                    if self.binding_name_is_global(name) {
                        let object_binding = self
                            .module
                            .global_prototype_object_bindings
                            .entry(name.clone())
                            .or_insert_with(empty_object_value_binding);
                        object_binding_set_property(
                            object_binding,
                            materialized_property,
                            materialized,
                        );
                    }
                    self.update_member_function_assignment_binding(object, property, value);
                    self.emit_numeric_expression(value)?;
                    return Ok(());
                }
                let static_array_property = if inline_summary_side_effect_free_expression(property)
                    && !self.expression_depends_on_active_loop_assignment(property)
                {
                    self.resolve_property_key_expression(property)
                        .unwrap_or_else(|| self.materialize_static_expression(property))
                } else {
                    property.as_ref().clone()
                };
                if let Expression::Identifier(name) = object.as_ref() {
                    if self.local_typed_array_view_bindings.contains_key(name) {
                        self.emit_typed_array_view_write(name, property, value)?;
                        return Ok(());
                    }
                    if let Some(realm_id) =
                        self.resolve_test262_realm_global_id_from_expression(object)
                    {
                        let materialized_property = self.materialize_static_expression(property);
                        let materialized = self.materialize_static_expression(value);
                        if let Some(realm) = self.module.test262_realms.get_mut(&realm_id) {
                            object_binding_set_property(
                                &mut realm.global_object_binding,
                                materialized_property,
                                materialized,
                            );
                            self.emit_numeric_expression(value)?;
                            return Ok(());
                        }
                    }
                    if let Some(index) = argument_index_from_expression(&static_array_property) {
                        let materialized = self.materialize_static_expression(value);
                        let length_local = self.runtime_array_length_locals.get(name).copied();
                        let use_global_runtime_array = self.is_named_global_array_binding(name)
                            && (!self.top_level_function
                                || self.uses_global_runtime_array_state(name));
                        let value_local = self.allocate_temp_local();
                        self.emit_numeric_expression(value)?;
                        self.push_local_set(value_local);
                        let mut array_length = None;
                        if let Some(array_binding) = self.local_array_bindings.get_mut(name) {
                            while array_binding.values.len() <= index as usize {
                                array_binding.values.push(None);
                            }
                            array_binding.values[index as usize] = Some(materialized.clone());
                            array_length = Some(array_binding.values.len() as i32);
                        } else if let Some(array_binding) =
                            self.module.global_array_bindings.get_mut(name)
                        {
                            while array_binding.values.len() <= index as usize {
                                array_binding.values.push(None);
                            }
                            array_binding.values[index as usize] = Some(materialized);
                            array_length = Some(array_binding.values.len() as i32);
                        }
                        if let Some(array_length) = array_length {
                            self.update_tracked_array_specialized_function_value(
                                name, index, value,
                            )?;
                            if !use_global_runtime_array && let Some(length_local) = length_local {
                                self.push_i32_const(array_length);
                                self.push_local_set(length_local);
                            }
                            if use_global_runtime_array {
                                if self.emit_global_runtime_array_slot_write_from_local(
                                    name,
                                    index,
                                    value_local,
                                )? {
                                    self.instructions.push(0x1a);
                                }
                            } else if self.emit_runtime_array_slot_write_from_local(
                                name,
                                index,
                                value_local,
                            )? {
                                self.instructions.push(0x1a);
                            }
                            self.push_local_get(value_local);
                            return Ok(());
                        }
                    }
                    if self.is_named_global_array_binding(name)
                        && (!self.top_level_function || self.uses_global_runtime_array_state(name))
                    {
                        if self
                            .emit_dynamic_global_runtime_array_slot_write(name, property, value)?
                        {
                            return Ok(());
                        }
                    } else if self.emit_dynamic_runtime_array_slot_write(name, property, value)? {
                        return Ok(());
                    }
                    let resolved_property =
                        if self.expression_depends_on_active_loop_assignment(property) {
                            self.materialize_static_expression(property)
                        } else {
                            self.resolve_property_key_expression(property)
                                .unwrap_or_else(|| self.materialize_static_expression(property))
                        };
                    if self.local_array_bindings.contains_key(name)
                        || self.module.global_array_bindings.contains_key(name)
                    {
                        let materialized = self.materialize_static_expression(value);
                        if self.local_array_bindings.contains_key(name) {
                            let object_binding = self
                                .local_object_bindings
                                .entry(name.clone())
                                .or_insert_with(empty_object_value_binding);
                            object_binding_set_property(
                                object_binding,
                                resolved_property.clone(),
                                materialized.clone(),
                            );
                        }
                        if self.module.global_array_bindings.contains_key(name) {
                            let object_binding = self
                                .module
                                .global_object_bindings
                                .entry(name.clone())
                                .or_insert_with(empty_object_value_binding);
                            object_binding_set_property(
                                object_binding,
                                resolved_property.clone(),
                                materialized,
                            );
                        }
                    }
                    if let Expression::String(property_name) = resolved_property
                        && self
                            .runtime_object_property_shadow_owner_name_for_identifier(name)
                            .is_some()
                    {
                        let value_local = self.allocate_temp_local();
                        self.emit_numeric_expression(value)?;
                        self.push_local_set(value_local);
                        self.emit_scoped_property_store_from_local(
                            object,
                            &property_name,
                            value_local,
                            value,
                        )?;
                        return Ok(());
                    }
                    let materialized_property = self.materialize_static_expression(property);
                    let materialized = self.materialize_static_expression(value);
                    if let Some(object_binding) = self.local_object_bindings.get_mut(name) {
                        object_binding_set_property(
                            object_binding,
                            materialized_property.clone(),
                            materialized.clone(),
                        );
                        self.update_member_function_assignment_binding(object, property, value);
                        self.emit_numeric_expression(value)?;
                        return Ok(());
                    }
                    if let Some(object_binding) = self.module.global_object_bindings.get_mut(name) {
                        object_binding_set_property(
                            object_binding,
                            materialized_property,
                            materialized,
                        );
                        self.update_member_function_assignment_binding(object, property, value);
                        self.emit_numeric_expression(value)?;
                        return Ok(());
                    }
                    if self
                        .resolve_function_binding_from_expression(object)
                        .is_some()
                    {
                        let object_binding = self
                            .local_object_bindings
                            .entry(name.clone())
                            .or_insert_with(empty_object_value_binding);
                        object_binding_set_property(
                            object_binding,
                            materialized_property.clone(),
                            materialized.clone(),
                        );
                        if self.binding_name_is_global(name) {
                            let global_binding = self
                                .module
                                .global_object_bindings
                                .entry(name.clone())
                                .or_insert_with(empty_object_value_binding);
                            object_binding_set_property(
                                global_binding,
                                materialized_property,
                                materialized,
                            );
                        }
                        self.local_kinds
                            .insert(name.clone(), StaticValueKind::Object);
                        self.update_member_function_assignment_binding(object, property, value);
                        self.emit_numeric_expression(value)?;
                        return Ok(());
                    }
                }
                self.emit_numeric_expression(object)?;
                self.instructions.push(0x1a);
                self.emit_numeric_expression(property)?;
                self.instructions.push(0x1a);
                self.emit_numeric_expression(value)?;
                self.instructions.push(0x1a);
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                Ok(())
            }
            Expression::AssignSuperMember { property, value } => {
                let runtime_prototype_binding = self
                    .resolve_super_runtime_prototype_binding_with_context(
                        self.current_user_function_name.as_deref(),
                    );
                let runtime_state_local = runtime_prototype_binding
                    .as_ref()
                    .and_then(|(_, binding)| binding.global_index)
                    .map(|global_index| {
                        let local = self.allocate_temp_local();
                        self.push_global_get(global_index);
                        self.push_local_set(local);
                        local
                    });

                let resolved_property = self.emit_property_key_expression_effects(property)?;
                let Some(effective_property) = resolved_property.as_ref() else {
                    self.emit_numeric_expression(value)?;
                    self.instructions.push(0x1a);
                    self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                    return Ok(());
                };
                let super_base = self.resolve_super_base_expression_with_context(
                    self.current_user_function_name.as_deref(),
                );

                if let Some((_, binding)) = runtime_prototype_binding.as_ref()
                    && let Some(state_local) = runtime_state_local
                    && let Some(variants) =
                        self.resolve_user_super_setter_variants(binding, effective_property)
                {
                    let value_local = self.allocate_temp_local();
                    self.emit_numeric_expression(value)?;
                    self.push_local_set(value_local);
                    self.emit_super_member_user_setter_call_via_runtime_prototype_state(
                        &variants,
                        state_local,
                        value_local,
                    )?;
                    self.push_local_get(value_local);
                    return Ok(());
                }

                if runtime_prototype_binding.is_none()
                    && let Some(super_base) = super_base.as_ref()
                    && let Some((user_function, capture_slots)) =
                        self.resolve_user_super_setter_call(super_base, effective_property)
                {
                    let value_local = self.allocate_temp_local();
                    self.emit_numeric_expression(value)?;
                    self.push_local_set(value_local);
                    self.emit_super_member_user_setter_call(
                        &user_function,
                        capture_slots.as_ref(),
                        value_local,
                    )?;
                    self.push_local_get(value_local);
                    return Ok(());
                }

                self.emit_numeric_expression(&Expression::AssignMember {
                    object: Box::new(Expression::This),
                    property: Box::new(effective_property.clone()),
                    value: value.clone(),
                })
            }
            Expression::This => {
                self.push_global_get(CURRENT_THIS_GLOBAL_INDEX);
                Ok(())
            }
            Expression::EnumerateKeys(expression) => {
                self.emit_numeric_expression(expression)?;
                self.instructions.push(0x1a);
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                Ok(())
            }
            Expression::GetIterator(expression) => {
                let materialized_expression = self.materialize_static_expression(expression);
                let iterator_target =
                    if !static_expression_matches(&materialized_expression, expression) {
                        &materialized_expression
                    } else {
                        expression.as_ref()
                    };
                if let Expression::Identifier(name) = expression.as_ref() {
                    if self.local_typed_array_view_bindings.contains_key(name) {
                        if let Some(oob_local) =
                            self.runtime_typed_array_oob_locals.get(name).copied()
                        {
                            self.push_local_get(oob_local);
                            self.instructions.push(0x04);
                            self.instructions.push(EMPTY_BLOCK_TYPE);
                            self.push_control_frame();
                            self.emit_named_error_throw("TypeError")?;
                            self.instructions.push(0x0b);
                            self.pop_control_frame();
                        }
                        self.emit_numeric_expression(expression)?;
                        self.instructions.push(0x1a);
                        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                        return Ok(());
                    }
                }
                if let Some((function_name, returned_expression, effect_statements)) =
                    self.analyze_effectful_iterator_source_call(iterator_target)
                {
                    let previous_strict_mode = self.strict_mode;
                    let previous_user_function_name = self.current_user_function_name.clone();
                    if let Some(user_function) = self.module.user_function_map.get(&function_name) {
                        self.strict_mode = user_function.strict;
                    }
                    self.current_user_function_name = Some(function_name);
                    for statement in &effect_statements {
                        self.emit_statement(statement)?;
                    }
                    self.strict_mode = previous_strict_mode;
                    self.current_user_function_name = previous_user_function_name;
                    return self.emit_numeric_expression(&Expression::GetIterator(Box::new(
                        returned_expression,
                    )));
                }
                if matches!(
                    self.infer_value_kind(iterator_target),
                    Some(StaticValueKind::Undefined | StaticValueKind::Null)
                ) {
                    return self.emit_named_error_throw("TypeError");
                }
                if matches!(
                    self.resolve_iterator_source_kind(iterator_target),
                    Some(IteratorSourceKind::SimpleGenerator { .. })
                ) {
                    self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                    return Ok(());
                }
                let has_next_method = self
                    .resolve_object_binding_from_expression(iterator_target)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(
                            &object_binding,
                            &Expression::String("next".to_string()),
                        )
                        .cloned()
                    })
                    .and_then(|value| self.resolve_function_binding_from_expression(&value))
                    .is_some()
                    || self
                        .resolve_member_function_binding(
                            iterator_target,
                            &Expression::String("next".to_string()),
                        )
                        .is_some();
                if has_next_method {
                    self.emit_numeric_expression(iterator_target)?;
                    return Ok(());
                }
                let iterator_property =
                    self.materialize_static_expression(&symbol_iterator_expression());
                if self
                    .resolve_member_function_binding(iterator_target, &iterator_property)
                    .is_some()
                    || self
                        .resolve_member_getter_binding(iterator_target, &iterator_property)
                        .is_some()
                {
                    return self.emit_numeric_expression(&Expression::Call {
                        callee: Box::new(Expression::Member {
                            object: Box::new(iterator_target.clone()),
                            property: Box::new(iterator_property),
                        }),
                        arguments: Vec::new(),
                    });
                }
                self.emit_numeric_expression(iterator_target)?;
                self.instructions.push(0x1a);
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                Ok(())
            }
            Expression::IteratorClose(expression) => {
                let return_property = Expression::String("return".to_string());
                let capture_source_bindings = self
                    .resolve_member_function_capture_source_bindings(expression, &return_property);
                let should_call_return = self
                    .resolve_object_binding_from_expression(expression)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(&object_binding, &return_property).cloned()
                    })
                    .map(|value| !matches!(value, Expression::Undefined | Expression::Null))
                    .unwrap_or_else(|| {
                        self.resolve_member_function_binding(expression, &return_property)
                            .is_some()
                            || self
                                .resolve_member_getter_binding(expression, &return_property)
                                .is_some()
                    });
                if let Expression::Identifier(name) = expression.as_ref()
                    && let Some(iterator_binding) =
                        self.local_array_iterator_bindings.get(name).cloned()
                {
                    let state_local = iterator_binding.index_local;
                    match iterator_binding.source {
                        IteratorSourceKind::SimpleGenerator { steps, .. }
                            if !should_call_return =>
                        {
                            let closed_state = (steps.len() + 1) as i32;
                            self.push_i32_const(closed_state);
                            self.push_local_set(state_local);
                            self.push_i32_const(JS_UNDEFINED_TAG);
                            return Ok(());
                        }
                        IteratorSourceKind::StaticArray { .. }
                        | IteratorSourceKind::TypedArrayView { .. }
                        | IteratorSourceKind::DirectArguments { .. }
                            if !should_call_return =>
                        {
                            self.push_i32_const(JS_UNDEFINED_TAG);
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                let should_call_return = self
                    .resolve_object_binding_from_expression(expression)
                    .and_then(|object_binding| {
                        object_binding_lookup_value(&object_binding, &return_property).cloned()
                    })
                    .map(|value| !matches!(value, Expression::Undefined | Expression::Null))
                    .unwrap_or_else(|| {
                        self.resolve_member_function_binding(expression, &return_property)
                            .is_some()
                            || self
                                .resolve_member_getter_binding(expression, &return_property)
                                .is_some()
                    });
                if should_call_return {
                    self.emit_numeric_expression(&Expression::Call {
                        callee: Box::new(Expression::Member {
                            object: Box::new((**expression).clone()),
                            property: Box::new(return_property),
                        }),
                        arguments: Vec::new(),
                    })?;
                    self.instructions.push(0x1a);
                    if !capture_source_bindings.is_empty() {
                        self.runtime_dynamic_bindings
                            .extend(capture_source_bindings);
                    }
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }
                self.emit_numeric_expression(expression)?;
                self.instructions.push(0x1a);
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            Expression::Await(expression) => {
                self.emit_numeric_expression(expression)?;
                self.instructions.push(0x1a);
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            Expression::New { callee, arguments } => {
                if let Expression::Identifier(name) = callee.as_ref() {
                    if name == "Proxy" && self.is_unshadowed_builtin_identifier(name) {
                        for argument in arguments {
                            match argument {
                                CallArgument::Expression(expression)
                                | CallArgument::Spread(expression) => {
                                    self.emit_numeric_expression(expression)?;
                                    self.instructions.push(0x1a);
                                }
                            }
                        }
                        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                        return Ok(());
                    }
                }

                if let Some(LocalFunctionBinding::User(function_name)) =
                    self.resolve_function_binding_from_expression(callee)
                {
                    if let Some(user_function) =
                        self.module.user_function_map.get(&function_name).cloned()
                    {
                        if self.emit_user_function_construct(callee, &user_function, arguments)? {
                            return Ok(());
                        }
                    }
                }

                if let Expression::Identifier(name) = callee.as_ref() {
                    if self.emit_builtin_call(name, arguments)? {
                        return Ok(());
                    }

                    if let Some(native_error_value) = native_error_runtime_value(name) {
                        for argument in arguments {
                            match argument {
                                CallArgument::Expression(expression)
                                | CallArgument::Spread(expression) => {
                                    self.emit_numeric_expression(expression)?;
                                    self.instructions.push(0x1a);
                                }
                            }
                        }
                        self.push_i32_const(native_error_value);
                        return Ok(());
                    }
                }
                self.emit_numeric_expression(callee)?;
                self.instructions.push(0x1a);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) => {
                            self.emit_numeric_expression(expression)?;
                        }
                        CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                        }
                    }
                    self.instructions.push(0x1a);
                }
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                Ok(())
            }
            Expression::Update { name, op, prefix } => {
                if let Some(scope_object) = self.resolve_with_scope_binding(name)? {
                    self.emit_scoped_property_update(&scope_object, name, *op, *prefix)?;
                    return Ok(());
                }

                let opcode = match op {
                    UpdateOp::Increment => 0x6a,
                    UpdateOp::Decrement => 0x6b,
                };

                let previous_kind = self
                    .lookup_identifier_kind(name)
                    .unwrap_or(StaticValueKind::Unknown);

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
                        self.emit_store_identifier_from_local(name, nan_local)?;
                        self.note_identifier_numeric_kind(name);
                        self.push_local_get(nan_local);
                        return Ok(());
                    }
                    StaticValueKind::Null => {
                        let previous_local = self.allocate_temp_local();
                        let next_local = self.allocate_temp_local();
                        self.push_i32_const(0);
                        self.push_local_set(previous_local);
                        self.push_i32_const(match op {
                            UpdateOp::Increment => 1,
                            UpdateOp::Decrement => -1,
                        });
                        self.push_local_set(next_local);
                        self.emit_store_identifier_from_local(name, next_local)?;
                        self.note_identifier_numeric_kind(name);
                        if *prefix {
                            self.push_local_get(next_local);
                        } else {
                            self.push_local_get(previous_local);
                        }
                        return Ok(());
                    }
                    _ => {}
                }

                if let Some((_, local_index)) = self.resolve_current_local_binding(name) {
                    if *prefix {
                        self.push_local_get(local_index);
                        self.push_i32_const(1);
                        self.instructions.push(opcode);
                        self.push_local_tee(local_index);
                    } else {
                        self.push_local_get(local_index);
                        self.push_local_get(local_index);
                        self.push_i32_const(1);
                        self.instructions.push(opcode);
                        self.push_local_set(local_index);
                    }
                } else if let Some(global_index) = self.module.global_bindings.get(name).copied() {
                    if *prefix {
                        let result_local = self.allocate_temp_local();
                        self.push_global_get(global_index);
                        self.push_i32_const(1);
                        self.instructions.push(opcode);
                        self.push_local_tee(result_local);
                        self.push_global_set(global_index);
                        self.push_local_get(result_local);
                    } else {
                        let previous_local = self.allocate_temp_local();
                        self.push_global_get(global_index);
                        self.push_local_tee(previous_local);
                        self.push_i32_const(1);
                        self.instructions.push(opcode);
                        self.push_global_set(global_index);
                        self.push_local_get(previous_local);
                    }
                } else if let Some(binding) =
                    self.module.implicit_global_bindings.get(name).copied()
                {
                    let previous_local = self.allocate_temp_local();
                    let next_local = self.allocate_temp_local();
                    self.push_global_get(binding.present_index);
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_global_get(binding.value_index);
                    self.push_local_tee(previous_local);
                    self.push_i32_const(1);
                    self.instructions.push(opcode);
                    self.push_local_tee(next_local);
                    self.push_global_set(binding.value_index);
                    self.instructions.push(0x05);
                    self.emit_named_error_throw("ReferenceError")?;
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                    if *prefix {
                        self.push_local_get(next_local);
                    } else {
                        self.push_local_get(previous_local);
                    }
                } else {
                    self.emit_named_error_throw("ReferenceError")?;
                }
                self.note_identifier_numeric_kind(name);
                Ok(())
            }
            Expression::Binary { op, left, right } => {
                if matches!(
                    op,
                    BinaryOp::Subtract
                        | BinaryOp::Multiply
                        | BinaryOp::Divide
                        | BinaryOp::Modulo
                        | BinaryOp::Exponentiate
                ) && let Some(number) = self.resolve_static_number_value(expression)
                {
                    return self.emit_numeric_expression(&Expression::Number(number));
                }
                match op {
                    BinaryOp::Add => {
                        let allow_static_addition = !(self.current_user_function_name.is_some()
                            && (self.addition_operand_requires_runtime_value(left)
                                || self.addition_operand_requires_runtime_value(right)));
                        if allow_static_addition
                            && let Some(outcome) = self
                                .resolve_static_addition_outcome_with_context(
                                    left,
                                    right,
                                    self.current_user_function_name.as_deref(),
                                )
                        {
                            return self.emit_static_eval_outcome(&outcome);
                        }
                        if let Some(text) = self.resolve_static_string_addition_value_with_context(
                            left,
                            right,
                            self.current_user_function_name.as_deref(),
                        ) {
                            self.emit_static_string_literal(&text)?;
                            return Ok(());
                        }
                        if self.emit_effectful_symbol_to_primitive_addition(left, right)? {
                            return Ok(());
                        }
                        if self.emit_effectful_ordinary_to_primitive_addition(left, right)? {
                            return Ok(());
                        }
                        self.emit_numeric_expression(left)?;
                        self.emit_numeric_expression(right)?;
                        self.push_binary_op(*op)
                    }
                    BinaryOp::LogicalAnd => self.emit_logical_and(left, right),
                    BinaryOp::LogicalOr => self.emit_logical_or(left, right),
                    BinaryOp::NullishCoalescing => self.emit_nullish_coalescing(left, right),
                    BinaryOp::Exponentiate => self.emit_exponentiate(left, right),
                    BinaryOp::Equal | BinaryOp::NotEqual
                        if self.emit_static_string_equality_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual
                        if self.emit_typeof_string_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual
                        if self.emit_runtime_typeof_tag_string_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual
                        if self.emit_hex_quad_string_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                        if self.emit_static_string_equality_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                        if self.emit_typeof_string_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                        if self.emit_runtime_typeof_tag_string_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::LooseEqual | BinaryOp::LooseNotEqual
                        if self.emit_hex_quad_string_comparison(left, right, *op)? =>
                    {
                        Ok(())
                    }
                    BinaryOp::LooseEqual => {
                        self.emit_loose_comparison(left, right)?;
                        self.instructions.push(0x46);
                        Ok(())
                    }
                    BinaryOp::LooseNotEqual => {
                        self.emit_loose_comparison(left, right)?;
                        self.instructions.push(0x47);
                        Ok(())
                    }
                    BinaryOp::In => {
                        self.emit_in_expression(left, right)?;
                        Ok(())
                    }
                    BinaryOp::InstanceOf => {
                        self.emit_instanceof_expression(left, right)?;
                        Ok(())
                    }
                    _ => {
                        self.emit_numeric_expression(left)?;
                        self.emit_numeric_expression(right)?;
                        self.push_binary_op(*op)
                    }
                }
            }
            Expression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                self.emit_numeric_expression(condition)?;
                self.instructions.push(0x04);
                self.instructions.push(I32_TYPE);
                self.push_control_frame();
                self.emit_numeric_expression(then_expression)?;
                self.instructions.push(0x05);
                self.emit_numeric_expression(else_expression)?;
                self.instructions.push(0x0b);
                self.pop_control_frame();
                Ok(())
            }
            Expression::Call { callee, arguments } => {
                self.last_bound_user_function_call = None;
                if let Some(number) = self.resolve_static_number_value(expression) {
                    return self.emit_numeric_expression(&Expression::Number(number));
                }
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && matches!(property.as_ref(), Expression::String(property_name) if property_name == "next")
                    && self.emit_fresh_simple_generator_next_call(object)?
                {
                    return Ok(());
                }
                if arguments.is_empty()
                    && let Expression::Member { object, property } = callee.as_ref()
                    && let Expression::String(property_name) = property.as_ref()
                    && let Some(outcome) = self.resolve_static_member_call_outcome_with_context(
                        object,
                        property_name,
                        self.current_user_function_name.as_deref(),
                    )
                {
                    return self.emit_static_eval_outcome(&outcome);
                }
                if self.emit_specialized_callee_call(callee, arguments)? {
                    return Ok(());
                }
                if self.emit_static_weakref_deref_call(callee, arguments)? {
                    return Ok(());
                }
                if self.emit_function_prototype_bind_call(callee, arguments)? {
                    return Ok(());
                }
                if let Expression::Member { object, property } = callee.as_ref() {
                    if self.emit_function_prototype_call_or_apply(object, property, arguments)? {
                        return Ok(());
                    }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == "assert")
                        && matches!(property.as_ref(), Expression::String(name) if name == "sameValue")
                        && self.emit_assertion_builtin_call("__assertSameValue", arguments)?
                    {
                        return Ok(());
                    }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == "assert")
                        && matches!(property.as_ref(), Expression::String(name) if name == "notSameValue")
                        && self.emit_assertion_builtin_call("__assertNotSameValue", arguments)?
                    {
                        return Ok(());
                    }
                    if self.emit_array_is_array_call(object, property, arguments)? {
                        return Ok(());
                    }
                    if self.emit_object_is_call(object, property, arguments)? {
                        return Ok(());
                    }
                    if self.emit_object_get_prototype_of_call(object, property, arguments)? {
                        return Ok(());
                    }
                    if self.emit_object_is_extensible_call(object, property, arguments)? {
                        return Ok(());
                    }
                    if self.emit_object_set_prototype_of_call(object, property, arguments)? {
                        return Ok(());
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "resize")
                    {
                        if let (
                            Expression::Identifier(buffer_name),
                            Some(
                                CallArgument::Expression(length_expression)
                                | CallArgument::Spread(length_expression),
                            ),
                        ) = (object.as_ref(), arguments.first())
                        {
                            if let Some(new_length) =
                                extract_typed_array_element_count(length_expression)
                            {
                                self.emit_numeric_expression(object)?;
                                self.instructions.push(0x1a);
                                self.emit_numeric_expression(length_expression)?;
                                self.instructions.push(0x1a);
                                for argument in arguments.iter().skip(1) {
                                    match argument {
                                        CallArgument::Expression(expression)
                                        | CallArgument::Spread(expression) => {
                                            self.emit_numeric_expression(expression)?;
                                            self.instructions.push(0x1a);
                                        }
                                    }
                                }
                                if self
                                    .apply_resizable_array_buffer_resize(buffer_name, new_length)?
                                {
                                    self.push_i32_const(JS_UNDEFINED_TAG);
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
                if let Expression::Identifier(name) = callee.as_ref() {
                    let resolved_local_name = self
                        .resolve_current_local_binding(name)
                        .map(|(resolved_name, _)| resolved_name);
                    if resolved_local_name.is_some()
                        || self.resolve_eval_local_function_hidden_name(name).is_some()
                    {
                        let binding_name = resolved_local_name.as_deref().unwrap_or(name);
                        if let Some(function_name) =
                            self.local_function_bindings.get(binding_name).cloned()
                        {
                            match function_name {
                                LocalFunctionBinding::User(function_name) => {
                                    if let Some(user_function) =
                                        self.module.user_function_map.get(&function_name).cloned()
                                    {
                                        self.emit_user_function_call(&user_function, arguments)?;
                                        return Ok(());
                                    }
                                }
                                LocalFunctionBinding::Builtin(function_name) => {
                                    if self.emit_builtin_call_for_callee(
                                        callee,
                                        &function_name,
                                        arguments,
                                    )? {
                                        return Ok(());
                                    }
                                    self.push_i32_const(JS_UNDEFINED_TAG);
                                    return Ok(());
                                }
                            }
                        }
                        if let Some(value) = self.local_value_bindings.get(binding_name).cloned() {
                            if let Some(function_binding) =
                                self.resolve_function_binding_from_expression(&value)
                            {
                                match function_binding {
                                    LocalFunctionBinding::User(function_name) => {
                                        if let Some(user_function) = self
                                            .module
                                            .user_function_map
                                            .get(&function_name)
                                            .cloned()
                                        {
                                            self.emit_user_function_call(
                                                &user_function,
                                                arguments,
                                            )?;
                                            return Ok(());
                                        }
                                    }
                                    LocalFunctionBinding::Builtin(function_name) => {
                                        if self.emit_builtin_call_for_callee(
                                            callee,
                                            &function_name,
                                            arguments,
                                        )? {
                                            return Ok(());
                                        }
                                        self.push_i32_const(JS_UNDEFINED_TAG);
                                        return Ok(());
                                    }
                                }
                            }
                        }

                        if self.emit_dynamic_user_function_call(callee, arguments)? {
                            return Ok(());
                        }
                        for argument in arguments {
                            match argument {
                                CallArgument::Expression(expression) => {
                                    self.emit_numeric_expression(expression)?;
                                    self.instructions.push(0x1a);
                                }
                                CallArgument::Spread(expression) => {
                                    self.emit_numeric_expression(expression)?;
                                    self.instructions.push(0x1a);
                                }
                            }
                        }
                        self.push_i32_const(JS_UNDEFINED_TAG);
                        return Ok(());
                    }

                    if name == "compareArray" && self.emit_compare_array_call(arguments)? {
                        return Ok(());
                    }

                    if name == "verifyProperty" && self.emit_verify_property_call(arguments)? {
                        return Ok(());
                    }
                    if name == "__ayyAssertThrows" && self.emit_assert_throws_call(arguments)? {
                        return Ok(());
                    }
                    if matches!(
                        name.as_str(),
                        "__assert" | "__assertSameValue" | "__assertNotSameValue"
                    ) && self.emit_builtin_call(name, arguments)?
                    {
                        return Ok(());
                    }

                    if let Some(function_binding) =
                        self.module.global_function_bindings.get(name).cloned()
                    {
                        match function_binding {
                            LocalFunctionBinding::User(function_name) => {
                                if let Some(user_function) =
                                    self.module.user_function_map.get(&function_name).cloned()
                                {
                                    self.emit_user_function_call(&user_function, arguments)?;
                                    return Ok(());
                                }
                            }
                            LocalFunctionBinding::Builtin(function_name) => {
                                if self.emit_builtin_call_for_callee(
                                    callee,
                                    &function_name,
                                    arguments,
                                )? {
                                    return Ok(());
                                }
                                self.push_i32_const(JS_UNDEFINED_TAG);
                                return Ok(());
                            }
                        }
                    }
                    if let Some(value) = self.module.global_value_bindings.get(name).cloned() {
                        if let Some(function_binding) =
                            self.resolve_function_binding_from_expression(&value)
                        {
                            match function_binding {
                                LocalFunctionBinding::User(function_name) => {
                                    if let Some(user_function) =
                                        self.module.user_function_map.get(&function_name).cloned()
                                    {
                                        self.emit_user_function_call(&user_function, arguments)?;
                                        return Ok(());
                                    }
                                }
                                LocalFunctionBinding::Builtin(function_name) => {
                                    if self.emit_builtin_call_for_callee(
                                        callee,
                                        &function_name,
                                        arguments,
                                    )? {
                                        return Ok(());
                                    }
                                    self.push_i32_const(JS_UNDEFINED_TAG);
                                    return Ok(());
                                }
                            }
                        }
                    }
                    if is_internal_user_function_identifier(name)
                        && let Some(user_function) =
                            self.module.user_function_map.get(name).cloned()
                    {
                        self.emit_user_function_call(&user_function, arguments)?;
                        return Ok(());
                    }
                    if self.emit_builtin_call_for_callee(callee, name, arguments)? {
                        return Ok(());
                    }

                    if self.emit_dynamic_user_function_call(callee, arguments)? {
                        return Ok(());
                    }
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                }

                if let Some(function_binding) =
                    self.resolve_function_binding_from_expression(callee)
                {
                    match function_binding {
                        LocalFunctionBinding::User(function_name) => {
                            if let Some(user_function) =
                                self.module.user_function_map.get(&function_name).cloned()
                            {
                                if let Expression::Member { object, property } = callee.as_ref() {
                                    let materialized_this_expression =
                                        self.materialize_static_expression(object);
                                    let materialized_call_arguments = arguments
                                        .iter()
                                        .map(|argument| match argument {
                                            CallArgument::Expression(expression)
                                            | CallArgument::Spread(expression) => {
                                                self.materialize_static_expression(expression)
                                            }
                                        })
                                        .collect::<Vec<_>>();
                                    if let Some(capture_slots) =
                                        self.resolve_member_function_capture_slots(object, property)
                                    {
                                        self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                                            &user_function,
                                            arguments,
                                            JS_UNDEFINED_TAG,
                                            object,
                                            &capture_slots,
                                        )?;
                                    } else {
                                        if self
                                            .can_inline_user_function_call_with_explicit_call_frame(
                                                &user_function,
                                                &materialized_call_arguments,
                                                &materialized_this_expression,
                                            )
                                        {
                                            let result_local = self.allocate_temp_local();
                                            if self.emit_inline_user_function_summary_with_explicit_call_frame(
                                                &user_function,
                                                &materialized_call_arguments,
                                                &materialized_this_expression,
                                                result_local,
                                            )? {
                                                self.push_local_get(result_local);
                                                return Ok(());
                                            }
                                        }
                                        self.emit_user_function_call_with_function_this_binding(
                                            &user_function,
                                            arguments,
                                            object,
                                            None,
                                        )?;
                                    }
                                } else if matches!(callee.as_ref(), Expression::SuperMember { .. })
                                {
                                    self.emit_user_function_call_with_new_target_and_this(
                                        &user_function,
                                        arguments,
                                        JS_UNDEFINED_TAG,
                                        JS_TYPEOF_OBJECT_TAG,
                                    )?;
                                } else {
                                    self.emit_user_function_call(&user_function, arguments)?;
                                }
                                return Ok(());
                            }
                        }
                        LocalFunctionBinding::Builtin(function_name) => {
                            if self.emit_builtin_call_for_callee(
                                callee,
                                &function_name,
                                arguments,
                            )? {
                                return Ok(());
                            }
                            self.push_i32_const(JS_UNDEFINED_TAG);
                            return Ok(());
                        }
                    }
                }

                if !matches!(callee.as_ref(), Expression::Member { .. })
                    && self.emit_dynamic_user_function_call(callee, arguments)?
                {
                    return Ok(());
                }

                if let Expression::Member { object, property } = callee.as_ref() {
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == "assert")
                        && matches!(property.as_ref(), Expression::String(name) if name == "compareArray")
                        && self.emit_assert_compare_array_call(arguments)?
                    {
                        return Ok(());
                    }
                    if self.emit_object_array_builtin_call(object, property, arguments)? {
                        return Ok(());
                    }
                    if self.emit_array_for_each_call(object, property, arguments)? {
                        return Ok(());
                    }
                    if let Some(function_binding) =
                        self.resolve_member_function_binding(object, property)
                    {
                        match function_binding {
                            LocalFunctionBinding::User(function_name) => {
                                if let Some(user_function) =
                                    self.module.user_function_map.get(&function_name).cloned()
                                {
                                    let materialized_this_expression =
                                        self.materialize_static_expression(object);
                                    let materialized_call_arguments = arguments
                                        .iter()
                                        .map(|argument| match argument {
                                            CallArgument::Expression(expression)
                                            | CallArgument::Spread(expression) => {
                                                self.materialize_static_expression(expression)
                                            }
                                        })
                                        .collect::<Vec<_>>();
                                    if let Some(capture_slots) =
                                        self.resolve_member_function_capture_slots(object, property)
                                    {
                                        self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                                            &user_function,
                                            arguments,
                                            JS_UNDEFINED_TAG,
                                            object,
                                            &capture_slots,
                                        )?;
                                    } else {
                                        if self
                                            .can_inline_user_function_call_with_explicit_call_frame(
                                                &user_function,
                                                &materialized_call_arguments,
                                                &materialized_this_expression,
                                            )
                                        {
                                            let result_local = self.allocate_temp_local();
                                            if self.emit_inline_user_function_summary_with_explicit_call_frame(
                                                &user_function,
                                                &materialized_call_arguments,
                                                &materialized_this_expression,
                                                result_local,
                                            )? {
                                                self.push_local_get(result_local);
                                                return Ok(());
                                            }
                                        }
                                        self.emit_user_function_call_with_function_this_binding(
                                            &user_function,
                                            arguments,
                                            object,
                                            None,
                                        )?;
                                    }
                                    return Ok(());
                                }
                            }
                            LocalFunctionBinding::Builtin(function_name) => {
                                if self.emit_builtin_call_for_callee(
                                    callee,
                                    &function_name,
                                    arguments,
                                )? {
                                    return Ok(());
                                }
                                self.push_i32_const(JS_UNDEFINED_TAG);
                                return Ok(());
                            }
                        }
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "push")
                    {
                        if let Expression::Identifier(name) = object.as_ref() {
                            let expanded_arguments = self.expand_call_arguments(arguments);
                            let materialized_arguments = expanded_arguments
                                .iter()
                                .map(|argument| self.materialize_static_expression(argument))
                                .collect::<Vec<_>>();
                            if self.local_array_bindings.contains_key(name)
                                || self.module.global_array_bindings.contains_key(name)
                            {
                                let use_global_runtime_array = self
                                    .is_named_global_array_binding(name)
                                    && (!self.top_level_function
                                        || self.uses_global_runtime_array_state(name));
                                self.emit_numeric_expression(object)?;
                                self.instructions.push(0x1a);
                                let argument_locals = expanded_arguments
                                    .iter()
                                    .map(|argument| {
                                        let local = self.allocate_temp_local();
                                        self.emit_numeric_expression(argument)?;
                                        self.push_local_set(local);
                                        Ok(local)
                                    })
                                    .collect::<DirectResult<Vec<_>>>()?;
                                for argument_local in &argument_locals {
                                    self.push_local_get(*argument_local);
                                    self.instructions.push(0x1a);
                                }
                                let mut old_length = None;
                                let mut new_length = None;
                                if let Some(array_binding) = self.local_array_bindings.get_mut(name)
                                {
                                    old_length = Some(array_binding.values.len() as u32);
                                    array_binding
                                        .values
                                        .extend(materialized_arguments.into_iter().map(Some));
                                    new_length = Some(array_binding.values.len() as i32);
                                } else if let Some(array_binding) =
                                    self.module.global_array_bindings.get_mut(name)
                                {
                                    old_length = Some(array_binding.values.len() as u32);
                                    array_binding
                                        .values
                                        .extend(materialized_arguments.into_iter().map(Some));
                                    new_length = Some(array_binding.values.len() as i32);
                                }
                                let mut used_runtime_push = false;
                                if let Some(old_length) = old_length {
                                    for (offset, argument_local) in
                                        argument_locals.iter().enumerate()
                                    {
                                        if !use_global_runtime_array
                                            && self.emit_runtime_array_push_from_local(
                                                name,
                                                *argument_local,
                                                &expanded_arguments[offset],
                                            )?
                                        {
                                            used_runtime_push = true;
                                            if offset + 1 < argument_locals.len() {
                                                self.instructions.push(0x1a);
                                            }
                                            continue;
                                        }
                                        self.update_tracked_array_specialized_function_value(
                                            name,
                                            old_length + offset as u32,
                                            &expanded_arguments[offset],
                                        )?;
                                        if use_global_runtime_array {
                                            if self
                                                .emit_global_runtime_array_slot_write_from_local(
                                                    name,
                                                    old_length + offset as u32,
                                                    *argument_local,
                                                )?
                                            {
                                                self.instructions.push(0x1a);
                                            }
                                        } else if self.emit_runtime_array_slot_write_from_local(
                                            name,
                                            old_length + offset as u32,
                                            *argument_local,
                                        )? {
                                            self.instructions.push(0x1a);
                                        }
                                    }
                                }
                                if used_runtime_push {
                                    return Ok(());
                                }
                                let new_length =
                                    new_length.expect("tracked push length should exist");
                                if !use_global_runtime_array
                                    && let Some(length_local) =
                                        self.runtime_array_length_locals.get(name).copied()
                                {
                                    self.push_i32_const(new_length);
                                    self.push_local_set(length_local);
                                }
                                if use_global_runtime_array {
                                    self.emit_global_runtime_array_length_write(name, new_length);
                                }
                                self.push_i32_const(new_length);
                                return Ok(());
                            }
                        }
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "pop")
                    {
                        if let Expression::Identifier(name) = object.as_ref() {
                            self.emit_numeric_expression(object)?;
                            self.instructions.push(0x1a);
                            let length_local = self.runtime_array_length_locals.get(name).copied();
                            let use_global_runtime_array = self.is_named_global_array_binding(name)
                                && (!self.top_level_function
                                    || self.uses_global_runtime_array_state(name));
                            let mut popped_value = None;
                            let mut popped_index = None;
                            let mut new_length = None;
                            if let Some(array_binding) = self.local_array_bindings.get_mut(name) {
                                popped_index = array_binding
                                    .values
                                    .len()
                                    .checked_sub(1)
                                    .map(|index| index as u32);
                                popped_value = Some(
                                    array_binding
                                        .values
                                        .pop()
                                        .flatten()
                                        .unwrap_or(Expression::Undefined),
                                );
                                new_length = Some(array_binding.values.len() as i32);
                            } else if let Some(array_binding) =
                                self.module.global_array_bindings.get_mut(name)
                            {
                                popped_index = array_binding
                                    .values
                                    .len()
                                    .checked_sub(1)
                                    .map(|index| index as u32);
                                popped_value = Some(
                                    array_binding
                                        .values
                                        .pop()
                                        .flatten()
                                        .unwrap_or(Expression::Undefined),
                                );
                                new_length = Some(array_binding.values.len() as i32);
                            }
                            if let Some(popped_index) = popped_index {
                                if use_global_runtime_array {
                                    self.clear_global_runtime_array_slot(name, popped_index);
                                } else {
                                    self.clear_runtime_array_slot(name, popped_index);
                                }
                            }
                            if let Some(new_length) = new_length {
                                if !use_global_runtime_array
                                    && let Some(length_local) = length_local
                                {
                                    self.push_i32_const(new_length);
                                    self.push_local_set(length_local);
                                }
                                if use_global_runtime_array {
                                    self.emit_global_runtime_array_length_write(name, new_length);
                                }
                                self.emit_numeric_expression(
                                    &popped_value.expect("tracked pop value should exist"),
                                )?;
                                return Ok(());
                            }
                        }
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "hasOwnProperty")
                    {
                        if let [CallArgument::Expression(argument_property)] = arguments.as_slice()
                        {
                            if let Some(array_binding) =
                                self.resolve_array_binding_from_expression(object)
                            {
                                let has_property = matches!(argument_property, Expression::String(property_name) if property_name == "length")
                                    || argument_index_from_expression(argument_property)
                                        .is_some_and(|index| {
                                            array_binding
                                                .values
                                                .get(index as usize)
                                                .is_some_and(|value| value.is_some())
                                        });
                                self.emit_numeric_expression(object)?;
                                self.instructions.push(0x1a);
                                self.emit_numeric_expression(argument_property)?;
                                self.instructions.push(0x1a);
                                self.push_i32_const(if has_property { 1 } else { 0 });
                                return Ok(());
                            }
                            if let Some(object_binding) =
                                self.resolve_object_binding_from_expression(object)
                            {
                                let has_property = self
                                    .resolve_object_binding_property_value(
                                        &object_binding,
                                        argument_property,
                                    )
                                    .is_some();
                                self.emit_numeric_expression(object)?;
                                self.instructions.push(0x1a);
                                self.emit_numeric_expression(argument_property)?;
                                self.instructions.push(0x1a);
                                self.push_i32_const(if has_property { 1 } else { 0 });
                                return Ok(());
                            }
                            if self
                                .resolve_user_function_from_expression(object)
                                .is_some_and(UserFunction::is_arrow)
                            {
                                if let Expression::String(property_name) = argument_property {
                                    if property_name == "caller" || property_name == "arguments" {
                                        self.emit_numeric_expression(object)?;
                                        self.instructions.push(0x1a);
                                        self.emit_numeric_expression(argument_property)?;
                                        self.instructions.push(0x1a);
                                        self.push_i32_const(0);
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "indexOf")
                    {
                        if let Expression::String(text) = object.as_ref() {
                            if let [CallArgument::Expression(Expression::String(search))] =
                                arguments.as_slice()
                            {
                                self.emit_numeric_expression(object)?;
                                self.instructions.push(0x1a);
                                self.emit_numeric_expression(&Expression::String(search.clone()))?;
                                self.instructions.push(0x1a);
                                self.push_i32_const(
                                    text.find(search).map(|index| index as i32).unwrap_or(-1),
                                );
                                return Ok(());
                            }
                        }
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "replace")
                        && inline_summary_side_effect_free_expression(object)
                        && let Some(source_text) = self.resolve_static_string_value(object)
                        && let [
                            CallArgument::Expression(search_expression),
                            CallArgument::Expression(replacement_expression),
                        ] = arguments.as_slice()
                        && inline_summary_side_effect_free_expression(search_expression)
                        && inline_summary_side_effect_free_expression(replacement_expression)
                        && let Some(search_text) =
                            self.resolve_static_string_value(search_expression)
                    {
                        self.emit_numeric_expression(object)?;
                        self.instructions.push(0x1a);
                        self.emit_numeric_expression(search_expression)?;
                        self.instructions.push(0x1a);
                        self.emit_numeric_expression(replacement_expression)?;
                        self.instructions.push(0x1a);

                        let replacement_text = if let Some(replacement_text) =
                            self.resolve_static_string_value(replacement_expression)
                        {
                            Some(replacement_text)
                        } else if let Some(LocalFunctionBinding::User(function_name)) =
                            self.resolve_function_binding_from_expression(replacement_expression)
                        {
                            let Some(user_function) =
                                self.module.user_function_map.get(&function_name).cloned()
                            else {
                                self.push_i32_const(JS_UNDEFINED_TAG);
                                return Ok(());
                            };
                            let Some(match_index) = source_text.find(&search_text) else {
                                self.emit_static_string_literal(&source_text)?;
                                return Ok(());
                            };
                            let callback_argument_expressions = vec![
                                Expression::String(search_text.clone()),
                                Expression::Number(match_index as f64),
                                Expression::String(source_text.clone()),
                            ];
                            let callback_arguments = callback_argument_expressions
                                .iter()
                                .cloned()
                                .map(CallArgument::Expression)
                                .collect::<Vec<_>>();
                            self.emit_user_function_call(&user_function, &callback_arguments)?;
                            self.instructions.push(0x1a);
                            let this_binding = if user_function.strict {
                                Expression::Undefined
                            } else {
                                Expression::This
                            };
                            self.resolve_function_binding_static_return_expression_with_call_frame(
                                &LocalFunctionBinding::User(function_name),
                                &callback_argument_expressions,
                                &this_binding,
                            )
                            .and_then(|value| self.resolve_static_string_value(&value))
                        } else {
                            None
                        };

                        if let Some(replacement_text) = replacement_text {
                            self.emit_static_string_literal(&source_text.replacen(
                                &search_text,
                                &replacement_text,
                                1,
                            ))?;
                            return Ok(());
                        }
                    }
                    if let Some(inlined_call) =
                        self.resolve_inline_call_from_returned_member(object, property, arguments)
                    {
                        self.emit_numeric_expression(object)?;
                        self.instructions.push(0x1a);
                        self.emit_numeric_expression(&inlined_call)?;
                        return Ok(());
                    }

                    if let Some(returned_value) =
                        self.resolve_returned_member_value_from_expression(object, property)
                    {
                        self.emit_numeric_expression(object)?;
                        self.instructions.push(0x1a);
                        if let Some(function_binding) =
                            self.resolve_function_binding_from_expression(&returned_value)
                        {
                            match function_binding {
                                LocalFunctionBinding::User(function_name) => {
                                    if let Some(user_function) =
                                        self.module.user_function_map.get(&function_name).cloned()
                                    {
                                        self.emit_user_function_call_with_new_target_and_this(
                                            &user_function,
                                            arguments,
                                            JS_UNDEFINED_TAG,
                                            JS_TYPEOF_OBJECT_TAG,
                                        )?;
                                        return Ok(());
                                    }
                                }
                                LocalFunctionBinding::Builtin(function_name) => {
                                    if self.emit_builtin_call_for_callee(
                                        callee,
                                        &function_name,
                                        arguments,
                                    )? {
                                        return Ok(());
                                    }
                                    self.push_i32_const(JS_UNDEFINED_TAG);
                                    return Ok(());
                                }
                            }
                        }
                    }
                }

                if self
                    .resolve_descriptor_binding_from_expression(expression)
                    .is_some()
                {
                    for argument in arguments {
                        match argument {
                            CallArgument::Expression(expression)
                            | CallArgument::Spread(expression) => {
                                self.emit_numeric_expression(expression)?;
                                self.instructions.push(0x1a);
                            }
                        }
                    }
                    self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                    return Ok(());
                }

                if let Expression::Member { object, property } = callee.as_ref() {
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "next")
                    {
                        if matches!(object.as_ref(), Expression::Identifier(name) if self.local_array_iterator_bindings.contains_key(name))
                        {
                            self.emit_numeric_expression(object)?;
                            self.instructions.push(0x1a);
                            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                            return Ok(());
                        }
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "slice")
                    {
                        if self
                            .resolve_array_slice_binding(object, arguments)
                            .is_some()
                        {
                            self.emit_numeric_expression(object)?;
                            self.instructions.push(0x1a);
                            for argument in arguments {
                                match argument {
                                    CallArgument::Expression(expression)
                                    | CallArgument::Spread(expression) => {
                                        self.emit_numeric_expression(expression)?;
                                        self.instructions.push(0x1a);
                                    }
                                }
                            }
                            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                            return Ok(());
                        }
                    }
                    if matches!(object.as_ref(), Expression::Identifier(name) if name == "Object")
                        && matches!(property.as_ref(), Expression::String(property_name) if property_name == "defineProperty")
                    {
                        if let [
                            CallArgument::Expression(target),
                            CallArgument::Expression(property_name_expression),
                            CallArgument::Expression(descriptor),
                            ..,
                        ] = arguments.as_slice()
                        {
                            if self.is_direct_arguments_object(target) {
                                if let Some(index) =
                                    argument_index_from_expression(property_name_expression)
                                {
                                    if let Some(descriptor) =
                                        resolve_property_descriptor_definition(descriptor)
                                    {
                                        if self.apply_direct_arguments_define_property(
                                            index,
                                            &descriptor,
                                        )? {
                                            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                                            return Ok(());
                                        }
                                    }
                                }
                            }

                            self.emit_numeric_expression(target)?;
                            self.instructions.push(0x1a);
                            self.emit_property_key_expression_effects(property_name_expression)?;
                            self.emit_numeric_expression(descriptor)?;
                            self.instructions.push(0x1a);
                            self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                            return Ok(());
                        }
                    }
                    if matches!(property.as_ref(), Expression::String(property_name) if property_name == "hasOwnProperty")
                    {
                        if let [CallArgument::Expression(argument_property)] = arguments.as_slice()
                        {
                            let direct_arguments = self.is_direct_arguments_object(object);
                            let arguments_binding =
                                self.resolve_arguments_binding_from_expression(object);
                            if direct_arguments {
                                match argument_property {
                                    Expression::String(owned_property_name) => {
                                        match owned_property_name.as_str() {
                                            "callee" | "length" => {
                                                self.push_i32_const(
                                                    if self.direct_arguments_has_property(
                                                        owned_property_name,
                                                    ) {
                                                        1
                                                    } else {
                                                        0
                                                    },
                                                );
                                                return Ok(());
                                            }
                                            _ => {
                                                if let Some(index) =
                                                    canonical_array_index_from_property_name(
                                                        owned_property_name,
                                                    )
                                                {
                                                    if let Some(slot) =
                                                        self.arguments_slots.get(&index)
                                                    {
                                                        self.push_local_get(slot.present_local);
                                                    } else {
                                                        self.push_i32_const(0);
                                                    }
                                                    return Ok(());
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if let Some(arguments_binding) = arguments_binding.as_ref() {
                                if let Expression::String(owned_property_name) = argument_property {
                                    let has_property = match owned_property_name.as_str() {
                                        "callee" => arguments_binding.callee_present,
                                        "length" => arguments_binding.length_present,
                                        _ => owned_property_name.parse::<usize>().ok().is_some_and(
                                            |index| index < arguments_binding.values.len(),
                                        ),
                                    };
                                    self.push_i32_const(if has_property { 1 } else { 0 });
                                    return Ok(());
                                }
                            }
                        }
                    }
                    if let Expression::Identifier(name) = object.as_ref() {
                        if let Some(descriptor) = self.local_descriptor_bindings.get(name) {
                            if matches!(property.as_ref(), Expression::String(property_name) if property_name == "hasOwnProperty")
                            {
                                if let [
                                    CallArgument::Expression(Expression::String(
                                        owned_property_name,
                                    )),
                                ] = arguments.as_slice()
                                {
                                    let has_property = match owned_property_name.as_str() {
                                        "configurable" | "enumerable" => true,
                                        "value" => descriptor.value.is_some(),
                                        "writable" => descriptor.writable.is_some(),
                                        "get" => descriptor.has_get,
                                        "set" => descriptor.has_set,
                                        _ => false,
                                    };
                                    self.push_i32_const(if has_property { 1 } else { 0 });
                                    return Ok(());
                                }
                            }
                        }
                    }
                }

                if matches!(callee.as_ref(), Expression::Member { .. })
                    && self.emit_dynamic_user_function_call(callee, arguments)?
                {
                    return Ok(());
                }

                self.emit_numeric_expression(callee)?;
                self.instructions.push(0x1a);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                        CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                            self.instructions.push(0x1a);
                        }
                    }
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
            Expression::Sequence(expressions) => {
                let Some((last, rest)) = expressions.split_last() else {
                    self.push_i32_const(JS_UNDEFINED_TAG);
                    return Ok(());
                };
                for expression in rest {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                }
                self.emit_numeric_expression(last)
            }
            Expression::SuperCall { callee, arguments } => {
                self.emit_numeric_expression(callee)?;
                self.instructions.push(0x1a);
                for argument in arguments {
                    match argument {
                        CallArgument::Expression(expression) => {
                            self.emit_numeric_expression(expression)?;
                        }
                        CallArgument::Spread(expression) => {
                            self.emit_numeric_expression(expression)?;
                        }
                    }
                    self.instructions.push(0x1a);
                }
                self.push_i32_const(JS_UNDEFINED_TAG);
                Ok(())
            }
        }
    }
}
