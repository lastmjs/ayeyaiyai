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
                if let Ok(parsed) = parse_string_to_i32(text) {
                    self.push_i32_const(parsed);
                } else {
                    self.emit_static_string_literal(text)?;
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
                            if let Expression::Identifier(name) = object.as_ref() {
                                let materialized_property =
                                    self.materialize_static_expression(property);
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
                    let inline_value = self.materialize_static_expression(value);
                    let value_local = self.allocate_temp_local();
                    self.emit_numeric_expression(object)?;
                    self.instructions.push(0x1a);
                    self.emit_numeric_expression(value)?;
                    self.push_local_set(value_local);
                    let callee = match function_binding {
                        LocalFunctionBinding::User(function_name)
                        | LocalFunctionBinding::Builtin(function_name) => {
                            Expression::Identifier(function_name)
                        }
                    };
                    if self.emit_arguments_slot_accessor_call(
                        &callee,
                        &[value_local],
                        1,
                        Some(std::slice::from_ref(&inline_value)),
                    )? {
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
                    if let Some(index) = argument_index_from_expression(property) {
                        let materialized = self.materialize_static_expression(value);
                        let length_local = self.runtime_array_length_locals.get(name).copied();
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
                            if let Some(length_local) = length_local {
                                self.push_i32_const(array_length);
                                self.push_local_set(length_local);
                            }
                            if self.emit_runtime_array_slot_write_from_local(
                                name,
                                index,
                                value_local,
                            )? {
                                return Ok(());
                            }
                            self.push_local_get(value_local);
                            return Ok(());
                        }
                    }
                    if self.emit_dynamic_runtime_array_slot_write(name, property, value)? {
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
                self.emit_numeric_expression(property)?;
                self.instructions.push(0x1a);
                self.emit_numeric_expression(value)?;
                self.instructions.push(0x1a);
                self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
                Ok(())
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
                if let Expression::Identifier(name) = expression.as_ref()
                    && let Some(iterator_binding) = self.local_array_iterator_bindings.get(name)
                {
                    let state_local = iterator_binding.index_local;
                    match &iterator_binding.source {
                        IteratorSourceKind::SimpleGenerator { steps, .. } => {
                            let closed_state = (steps.len() + 1) as i32;
                            self.push_i32_const(closed_state);
                            self.push_local_set(state_local);
                            self.push_i32_const(JS_UNDEFINED_TAG);
                            return Ok(());
                        }
                        IteratorSourceKind::StaticArray { .. }
                        | IteratorSourceKind::TypedArrayView { .. }
                        | IteratorSourceKind::DirectArguments { .. } => {
                            self.push_i32_const(i32::MAX);
                            self.push_local_set(state_local);
                            self.push_i32_const(JS_UNDEFINED_TAG);
                            return Ok(());
                        }
                    }
                }
                let return_property = Expression::String("return".to_string());
                if let Some(function_binding) =
                    self.resolve_member_function_binding(expression, &return_property)
                {
                    self.emit_numeric_expression(expression)?;
                    self.instructions.push(0x1a);
                    match function_binding {
                        LocalFunctionBinding::User(function_name) => {
                            if let Some(user_function) =
                                self.module.user_function_map.get(&function_name).cloned()
                            {
                                self.emit_user_function_call_without_inline_with_new_target_and_this(
                                    &user_function,
                                    &[],
                                    JS_UNDEFINED_TAG,
                                    JS_TYPEOF_OBJECT_TAG,
                                )?;
                                self.instructions.push(0x1a);
                                self.push_i32_const(JS_UNDEFINED_TAG);
                                return Ok(());
                            }
                        }
                        LocalFunctionBinding::Builtin(function_name) => {
                            if self.emit_builtin_call(&function_name, &[])? {
                                self.instructions.push(0x1a);
                                self.push_i32_const(JS_UNDEFINED_TAG);
                                return Ok(());
                            }
                        }
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
                        if self.emit_user_function_construct(&user_function, arguments)? {
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
            Expression::Binary { op, left, right } => match op {
                BinaryOp::Add => {
                    let allow_static_addition = !(self.current_user_function_name.is_some()
                        && (self.addition_operand_requires_runtime_value(left)
                            || self.addition_operand_requires_runtime_value(right)));
                    if allow_static_addition
                        && let Some(outcome) = self.resolve_static_addition_outcome_with_context(
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
            },
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
                                if matches!(
                                    callee.as_ref(),
                                    Expression::Member { .. } | Expression::SuperMember { .. }
                                ) {
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
                                    self.emit_user_function_call_with_new_target_and_this_expression(
                                        &user_function,
                                        arguments,
                                        JS_UNDEFINED_TAG,
                                        object,
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
                                        if self.emit_runtime_array_push_from_local(
                                            name,
                                            *argument_local,
                                            &expanded_arguments[offset],
                                        )? {
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
                                        if self.emit_runtime_array_slot_write_from_local(
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
                                if let Some(length_local) =
                                    self.runtime_array_length_locals.get(name).copied()
                                {
                                    self.push_i32_const(new_length);
                                    self.push_local_set(length_local);
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
                                self.clear_runtime_array_slot(name, popped_index);
                            }
                            if let Some(new_length) = new_length {
                                if let Some(length_local) = length_local {
                                    self.push_i32_const(new_length);
                                    self.push_local_set(length_local);
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

    pub(in crate::backend::direct_wasm) fn emit_logical_and(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        let temp_local = self.allocate_temp_local();
        self.emit_numeric_expression(left)?;
        self.push_local_set(temp_local);
        self.push_local_get(temp_local);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.emit_numeric_expression(right)?;
        self.instructions.push(0x05);
        self.push_local_get(temp_local);
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_logical_or(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        let temp_local = self.allocate_temp_local();
        self.emit_numeric_expression(left)?;
        self.push_local_set(temp_local);
        self.push_local_get(temp_local);
        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();
        self.push_local_get(temp_local);
        self.instructions.push(0x05);
        self.emit_numeric_expression(right)?;
        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_exponentiate(
        &mut self,
        base: &Expression,
        exponent: &Expression,
    ) -> DirectResult<()> {
        let base_local = self.allocate_temp_local();
        let result_local = self.allocate_temp_local();
        let exponent_local = self.allocate_temp_local();

        self.emit_numeric_expression(base)?;
        self.push_local_set(base_local);

        if let Expression::Number(power) = exponent {
            let power = f64_to_i32(*power)?;
            if power < 0 {
                self.push_i32_const(0);
            } else {
                self.push_i32_const(power);
            }
        } else {
            self.emit_numeric_expression(exponent)?;
        }
        self.push_local_set(exponent_local);

        self.push_i32_const(1);
        self.push_local_set(result_local);

        self.instructions.push(0x02);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let break_target = self.push_control_frame();

        self.instructions.push(0x03);
        self.instructions.push(EMPTY_BLOCK_TYPE);
        let loop_target = self.push_control_frame();

        self.push_local_get(exponent_local);
        self.push_i32_const(0);
        self.push_binary_op(BinaryOp::LessThanOrEqual)?;
        self.push_br_if(self.relative_depth(break_target));

        self.push_local_get(result_local);
        self.push_local_get(base_local);
        self.instructions.push(0x6c);
        self.push_local_set(result_local);

        self.push_local_get(exponent_local);
        self.push_i32_const(1);
        self.instructions.push(0x6b);
        self.push_local_set(exponent_local);

        self.push_br(self.relative_depth(loop_target));
        self.instructions.push(0x0b);
        self.pop_control_frame();
        self.instructions.push(0x0b);
        self.pop_control_frame();

        self.push_local_get(result_local);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_nullish_coalescing(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        let temp_local = self.allocate_temp_local();

        self.emit_numeric_expression(left)?;
        self.push_local_set(temp_local);

        self.push_local_get(temp_local);
        self.push_i32_const(JS_NULL_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;

        self.push_local_get(temp_local);
        self.push_i32_const(JS_UNDEFINED_TAG);
        self.push_binary_op(BinaryOp::NotEqual)?;
        self.instructions.push(0x71);

        self.instructions.push(0x04);
        self.instructions.push(I32_TYPE);
        self.push_control_frame();

        self.push_local_get(temp_local);

        self.instructions.push(0x05);
        self.emit_numeric_expression(right)?;

        self.instructions.push(0x0b);
        self.pop_control_frame();
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn push_binary_op(
        &mut self,
        op: BinaryOp,
    ) -> DirectResult<()> {
        let opcode = match op {
            BinaryOp::Add => 0x6a,
            BinaryOp::Subtract => 0x6b,
            BinaryOp::Multiply => 0x6c,
            BinaryOp::Divide => 0x6d,
            BinaryOp::Modulo => 0x6f,
            BinaryOp::Equal => 0x46,
            BinaryOp::NotEqual => 0x47,
            BinaryOp::LessThan => 0x48,
            BinaryOp::GreaterThan => 0x4a,
            BinaryOp::LessThanOrEqual => 0x4c,
            BinaryOp::GreaterThanOrEqual => 0x4e,
            BinaryOp::BitwiseAnd => 0x71,
            BinaryOp::BitwiseOr => 0x72,
            BinaryOp::BitwiseXor => 0x73,
            BinaryOp::LeftShift => 0x74,
            BinaryOp::RightShift => 0x75,
            BinaryOp::UnsignedRightShift => 0x76,
            _ => {
                self.push_i32_const(0);
                return Ok(());
            }
        };
        self.instructions.push(opcode);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn lookup_local(&self, name: &str) -> DirectResult<u32> {
        Ok(self.locals.get(name).copied().unwrap_or(self.param_count))
    }

    pub(in crate::backend::direct_wasm) fn emit_loose_comparison(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        self.emit_loose_number(left)?;
        self.emit_loose_number(right)?;
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_in_expression(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        if let Some(array_binding) = self.resolve_array_binding_from_expression(right) {
            if matches!(left, Expression::String(property_name) if property_name == "length") {
                self.push_i32_const(1);
                return Ok(());
            }
            if let Some(index) = argument_index_from_expression(left) {
                self.push_i32_const(
                    if array_binding
                        .values
                        .get(index as usize)
                        .is_some_and(|value| value.is_some())
                    {
                        1
                    } else {
                        0
                    },
                );
                return Ok(());
            }
        }
        if let Some(object_binding) = self.resolve_object_binding_from_expression(right) {
            let materialized_left = self.materialize_static_expression(left);
            self.push_i32_const(
                if object_binding_has_property(&object_binding, &materialized_left) {
                    1
                } else {
                    0
                },
            );
            return Ok(());
        }
        if let Expression::Identifier(name) = right {
            if let Expression::String(property_name) = left {
                let has_property = match name.as_str() {
                    "Number" => matches!(
                        property_name.as_str(),
                        "MAX_VALUE"
                            | "MIN_VALUE"
                            | "NaN"
                            | "POSITIVE_INFINITY"
                            | "NEGATIVE_INFINITY"
                    ),
                    _ => false,
                };
                if has_property {
                    self.push_i32_const(1);
                    return Ok(());
                }
            }
        }
        self.emit_numeric_expression(left)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(right)?;
        self.instructions.push(0x1a);
        self.push_i32_const(0);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn expression_is_builtin_array_constructor(
        &self,
        expression: &Expression,
    ) -> bool {
        matches!(
            self.materialize_static_expression(expression),
            Expression::Identifier(name) if name == "Array"
        )
    }

    pub(in crate::backend::direct_wasm) fn expression_is_known_array_value(
        &self,
        expression: &Expression,
    ) -> bool {
        if self
            .resolve_array_binding_from_expression(expression)
            .is_some()
        {
            return true;
        }

        let materialized = self.materialize_static_expression(expression);
        if !static_expression_matches(&materialized, expression)
            && self
                .resolve_array_binding_from_expression(&materialized)
                .is_some()
        {
            return true;
        }

        self.resolve_bound_alias_expression(expression)
            .filter(|resolved| !static_expression_matches(resolved, expression))
            .is_some_and(|resolved| self.expression_is_known_array_value(&resolved))
    }

    pub(in crate::backend::direct_wasm) fn emit_instanceof_expression(
        &mut self,
        left: &Expression,
        right: &Expression,
    ) -> DirectResult<()> {
        if self.expression_is_builtin_array_constructor(right) {
            self.push_i32_const(if self.expression_is_known_array_value(left) {
                1
            } else {
                0
            });
            return Ok(());
        }

        let materialized_right = self.materialize_static_expression(right);
        if let Expression::Identifier(name) = &materialized_right {
            if let Some(expected_values) = native_error_instanceof_values(name) {
                let left_local = self.allocate_temp_local();
                self.emit_numeric_expression(left)?;
                self.push_local_set(left_local);
                if let [expected_value] = expected_values.as_slice() {
                    self.push_local_get(left_local);
                    self.push_i32_const(*expected_value);
                    self.push_binary_op(BinaryOp::Equal)?;
                    return Ok(());
                }

                let matched_local = self.allocate_temp_local();
                self.push_i32_const(0);
                self.push_local_set(matched_local);
                for expected_value in expected_values {
                    self.push_local_get(left_local);
                    self.push_i32_const(expected_value);
                    self.push_binary_op(BinaryOp::Equal)?;
                    self.instructions.push(0x04);
                    self.instructions.push(EMPTY_BLOCK_TYPE);
                    self.push_control_frame();
                    self.push_i32_const(1);
                    self.push_local_set(matched_local);
                    self.instructions.push(0x0b);
                    self.pop_control_frame();
                }
                self.push_local_get(matched_local);
                return Ok(());
            }
        }

        self.emit_numeric_expression(left)?;
        self.instructions.push(0x1a);
        self.emit_numeric_expression(&materialized_right)?;
        self.instructions.push(0x1a);
        self.push_i32_const(0);
        Ok(())
    }

    pub(in crate::backend::direct_wasm) fn emit_loose_number(
        &mut self,
        expression: &Expression,
    ) -> DirectResult<()> {
        match expression {
            Expression::Null => {
                self.push_i32_const(0);
                Ok(())
            }
            Expression::Undefined => {
                self.push_i32_const(0);
                Ok(())
            }
            Expression::String(text) => {
                if let Ok(parsed) = parse_string_to_loose_i32(text) {
                    self.push_i32_const(parsed);
                } else {
                    self.emit_static_string_literal(text)?;
                }
                Ok(())
            }
            _ => self.emit_numeric_expression(expression),
        }
    }

    pub(in crate::backend::direct_wasm) fn find_labeled_loop_index(
        &self,
        label: &str,
    ) -> DirectResult<Option<usize>> {
        Ok(self
            .loop_stack
            .iter()
            .rposition(|loop_context| loop_context.labels.iter().any(|name| name == label)))
    }

    pub(in crate::backend::direct_wasm) fn break_hook_for_target(
        &self,
        break_target: usize,
    ) -> DirectResult<Option<Expression>> {
        for break_context in self.break_stack.iter().rev() {
            if break_context.break_target == break_target {
                return Ok(break_context.break_hook.clone());
            }
        }
        Ok(None)
    }

    pub(in crate::backend::direct_wasm) fn find_labeled_break(
        &self,
        label: &str,
    ) -> DirectResult<Option<usize>> {
        Ok(self
            .break_stack
            .iter()
            .rposition(|break_context| break_context.labels.iter().any(|name| name == label)))
    }

    pub(in crate::backend::direct_wasm) fn allocate_temp_local(&mut self) -> u32 {
        let local_index = self.next_local_index;
        self.next_local_index += 1;
        local_index
    }

    pub(in crate::backend::direct_wasm) fn push_control_frame(&mut self) -> usize {
        self.control_stack.push(());
        self.control_stack.len() - 1
    }

    pub(in crate::backend::direct_wasm) fn pop_control_frame(&mut self) {
        self.control_stack.pop();
    }

    pub(in crate::backend::direct_wasm) fn relative_depth(&self, target: usize) -> u32 {
        (self.control_stack.len() - 1 - target) as u32
    }

    pub(in crate::backend::direct_wasm) fn push_i32_const(&mut self, value: i32) {
        self.instructions.push(0x41);
        push_i32(&mut self.instructions, value);
    }

    pub(in crate::backend::direct_wasm) fn push_local_get(&mut self, local_index: u32) {
        self.instructions.push(0x20);
        push_u32(&mut self.instructions, local_index);
    }

    pub(in crate::backend::direct_wasm) fn push_local_set(&mut self, local_index: u32) {
        self.instructions.push(0x21);
        push_u32(&mut self.instructions, local_index);
    }

    pub(in crate::backend::direct_wasm) fn push_global_get(&mut self, global_index: u32) {
        self.instructions.push(0x23);
        push_u32(&mut self.instructions, global_index);
    }

    pub(in crate::backend::direct_wasm) fn push_global_set(&mut self, global_index: u32) {
        self.instructions.push(0x24);
        push_u32(&mut self.instructions, global_index);
    }

    pub(in crate::backend::direct_wasm) fn push_local_tee(&mut self, local_index: u32) {
        self.instructions.push(0x22);
        push_u32(&mut self.instructions, local_index);
    }

    pub(in crate::backend::direct_wasm) fn push_call(&mut self, function_index: u32) {
        self.instructions.push(0x10);
        push_u32(&mut self.instructions, function_index);
    }

    pub(in crate::backend::direct_wasm) fn push_br(&mut self, relative_depth: u32) {
        self.instructions.push(0x0c);
        push_u32(&mut self.instructions, relative_depth);
    }

    pub(in crate::backend::direct_wasm) fn push_br_if(&mut self, relative_depth: u32) {
        self.instructions.push(0x0d);
        push_u32(&mut self.instructions, relative_depth);
    }
}
