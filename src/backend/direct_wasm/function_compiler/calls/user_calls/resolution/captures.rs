use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn initialize_user_function_capture_slots_from_expression(
        &mut self,
        expression: &Expression,
        user_function: &UserFunction,
    ) -> DirectResult<Option<BTreeMap<String, String>>> {
        if let Some(capture_slots) = self.resolve_function_expression_capture_slots(expression) {
            return Ok(Some(capture_slots));
        }
        let Some(capture_bindings) = self
            .backend
            .function_registry
            .analysis
            .user_function_capture_bindings
            .get(&user_function.name)
            .filter(|captures| !captures.is_empty())
            .cloned()
        else {
            return Ok(None);
        };
        let Some(capture_source_bindings) =
            self.resolve_constructor_capture_source_bindings_from_expression(expression)
        else {
            return Ok(None);
        };

        let mut capture_slots = BTreeMap::new();
        for capture_name in capture_bindings.keys() {
            let Some(source_expression) = capture_source_bindings.get(capture_name).cloned() else {
                return Ok(None);
            };
            let hidden_name = self.allocate_named_hidden_local(
                &format!("closure_slot_{}_{}", user_function.name, capture_name),
                self.infer_value_kind(&source_expression)
                    .unwrap_or(StaticValueKind::Unknown),
            );
            let hidden_local = self
                .state
                .runtime
                .locals
                .get(&hidden_name)
                .copied()
                .expect("fresh returned function capture slot local must exist");
            self.emit_numeric_expression(&source_expression)?;
            self.push_local_set(hidden_local);
            self.update_capture_slot_binding_from_expression(&hidden_name, &source_expression)?;
            if let Expression::Identifier(source_binding_name) = &source_expression {
                self.state
                    .speculation
                    .static_semantics
                    .capture_slot_source_bindings
                    .insert(hidden_name.clone(), source_binding_name.clone());
            }
            capture_slots.insert(capture_name.clone(), hidden_name);
        }

        Ok(Some(capture_slots))
    }

    pub(in crate::backend::direct_wasm) fn emit_user_function_construct(
        &mut self,
        callee: &Expression,
        user_function: &UserFunction,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if !user_function.is_constructible() {
            return Ok(false);
        }

        let capture_slots =
            self.initialize_user_function_capture_slots_from_expression(callee, user_function)?;
        let capture_source_bindings = capture_slots
            .as_ref()
            .and_then(|_| self.resolve_constructor_capture_source_bindings_from_expression(callee));

        self.state
            .speculation
            .static_semantics
            .last_bound_user_function_call = Some(BoundUserFunctionCallSnapshot {
            function_name: user_function.name.clone(),
            source_expression: Some(Expression::New {
                callee: Box::new(callee.clone()),
                arguments: arguments.to_vec(),
            }),
            result_expression: self
                .resolve_user_constructor_object_binding_for_function(
                    user_function,
                    arguments,
                    capture_source_bindings.as_ref(),
                )
                .map(|binding| object_binding_to_expression(&binding)),
            updated_bindings: HashMap::new(),
        });

        if let Some(capture_slots) = capture_slots.as_ref() {
            self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                user_function,
                arguments,
                user_function_runtime_value(user_function),
                if self.user_function_is_derived_constructor(user_function) {
                    &Expression::Undefined
                } else {
                    &Expression::This
                },
                capture_slots,
            )?;
        } else {
            self.emit_user_function_call_with_new_target_and_this(
                user_function,
                arguments,
                user_function_runtime_value(user_function),
                if self.user_function_is_derived_constructor(user_function) {
                    JS_UNDEFINED_TAG
                } else {
                    JS_TYPEOF_OBJECT_TAG
                },
            )?;
        }
        self.state.emission.output.instructions.push(0x1a);
        self.push_i32_const(JS_TYPEOF_OBJECT_TAG);
        Ok(true)
    }
}
