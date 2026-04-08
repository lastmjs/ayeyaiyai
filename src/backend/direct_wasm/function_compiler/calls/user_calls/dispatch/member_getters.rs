use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_member_getter_returned_user_function(
        &self,
        object: &Expression,
        property: &Expression,
    ) -> Option<(UserFunction, BTreeMap<String, String>)> {
        let getter_binding = self.resolve_member_getter_binding(object, property)?;
        let returned_expression = self
            .resolve_function_binding_static_return_expression_with_call_frame(
                &getter_binding,
                &[],
                object,
            )?;
        let LocalFunctionBinding::User(function_name) =
            self.resolve_function_binding_from_expression(&returned_expression)?
        else {
            return None;
        };
        let user_function = self.user_function(&function_name)?.clone();
        let Some(captures) = self
            .backend
            .function_registry
            .analysis
            .user_function_capture_bindings
            .get(&user_function.name)
        else {
            return Some((user_function, BTreeMap::new()));
        };
        let getter_capture_slots = self
            .resolve_member_function_capture_slots(object, property)
            .or_else(|| self.resolve_function_expression_capture_slots(&returned_expression));
        let Some(getter_capture_slots) = getter_capture_slots else {
            return if captures.is_empty() {
                Some((user_function, BTreeMap::new()))
            } else {
                None
            };
        };
        let mut bound_capture_slots = BTreeMap::new();
        for capture_name in captures.keys() {
            let slot_name = getter_capture_slots.get(capture_name)?;
            bound_capture_slots.insert(capture_name.clone(), slot_name.clone());
        }
        Some((user_function, bound_capture_slots))
    }

    pub(in crate::backend::direct_wasm) fn emit_member_getter_returned_user_function_call(
        &mut self,
        object: &Expression,
        property: &Expression,
        arguments: &[CallArgument],
    ) -> DirectResult<bool> {
        if self.promise_member_call_requires_runtime_fallback(object, property, arguments) {
            return Ok(false);
        }
        let Some(LocalFunctionBinding::User(getter_function_name)) =
            self.resolve_member_getter_binding(object, property)
        else {
            return Ok(false);
        };
        let Some(getter_user_function) = self.user_function(&getter_function_name).cloned() else {
            return Ok(false);
        };
        let Some((returned_user_function, returned_capture_slots)) =
            self.resolve_member_getter_returned_user_function(object, property)
        else {
            return Ok(false);
        };

        let getter_capture_slots = self.resolve_member_function_capture_slots(object, property);
        self.emit_user_function_call_with_function_this_binding(
            &getter_user_function,
            &[],
            object,
            getter_capture_slots.as_ref(),
        )?;
        self.state.emission.output.instructions.push(0x1a);

        if returned_capture_slots.is_empty() {
            self.emit_user_function_call_with_function_this_binding(
                &returned_user_function,
                arguments,
                object,
                None,
            )?;
        } else {
            self.emit_user_function_call_with_new_target_and_this_expression_and_bound_captures(
                &returned_user_function,
                arguments,
                JS_UNDEFINED_TAG,
                object,
                &returned_capture_slots,
            )?;
        }
        Ok(true)
    }
}
