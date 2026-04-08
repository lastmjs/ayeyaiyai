use super::*;

struct PreparedIdentifierStoreState {
    canonical_value_expression: Expression,
    tracked_value_expression: Expression,
    descriptor_binding_expression: Expression,
    tracked_object_expression: Expression,
    function_binding_expression: Expression,
    function_binding: Option<LocalFunctionBinding>,
    object_binding_expression: Expression,
    kind: Option<StaticValueKind>,
    static_string_value: Option<String>,
    exact_static_number: Option<f64>,
    array_binding: Option<ArrayValueBinding>,
    module_assignment_expression: Expression,
    resolved_local_binding: Option<(String, u32)>,
    returned_descriptor_binding: Option<PropertyDescriptorBinding>,
    resolved_name: String,
    is_internal_array_iterator_binding: bool,
    is_internal_array_step_binding: bool,
    is_internal_iterator_temp: bool,
    iterator_binding_source: Option<IteratorSourceKind>,
}

#[path = "store_paths/capture_paths.rs"]
mod capture_paths;
#[path = "store_paths/common_updates.rs"]
mod common_updates;
#[path = "store_paths/global_paths.rs"]
mod global_paths;
#[path = "store_paths/local_paths.rs"]
mod local_paths;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn store_prepared_identifier_value_local(
        &mut self,
        name: &str,
        value_local: u32,
        prepared: PreparedIdentifierValueStore,
    ) -> DirectResult<()> {
        let PreparedIdentifierValueStore {
            canonical_value_expression,
            tracked_value_expression,
            descriptor_binding_expression,
            tracked_object_expression,
            call_source_snapshot_expression,
            function_binding_expression,
            function_binding,
            object_binding_expression,
            kind,
            static_string_value,
            exact_static_number,
            array_binding,
            module_assignment_expression,
            resolved_local_binding,
            returned_descriptor_binding,
        } = prepared;

        let resolved_name = resolved_local_binding
            .as_ref()
            .map(|(resolved_name, _)| resolved_name.as_str())
            .unwrap_or(name)
            .to_string();
        let is_internal_array_iterator_binding = resolved_name.starts_with("__ayy_array_iter_");
        let is_internal_array_step_binding = resolved_name.starts_with("__ayy_array_step_");
        let is_internal_iterator_temp =
            is_internal_array_iterator_binding || is_internal_array_step_binding;
        let iterator_binding_source = {
            let iterator_source_expression = match call_source_snapshot_expression
                .as_ref()
                .unwrap_or(&canonical_value_expression)
            {
                Expression::GetIterator(_) | Expression::Call { .. }
                    if self
                        .resolve_simple_generator_source(
                            call_source_snapshot_expression
                                .as_ref()
                                .unwrap_or(&canonical_value_expression),
                        )
                        .is_some()
                        || self
                            .resolve_async_yield_delegate_generator_plan(
                                call_source_snapshot_expression
                                    .as_ref()
                                    .unwrap_or(&canonical_value_expression),
                                "__ayy_async_delegate_completion",
                            )
                            .is_some() =>
                {
                    call_source_snapshot_expression
                        .as_ref()
                        .unwrap_or(&canonical_value_expression)
                }
                _ => &tracked_value_expression,
            };
            self.resolve_local_array_iterator_source(iterator_source_expression)
        };
        let state = PreparedIdentifierStoreState {
            canonical_value_expression,
            tracked_value_expression,
            descriptor_binding_expression,
            tracked_object_expression,
            function_binding_expression,
            function_binding,
            object_binding_expression,
            kind,
            static_string_value,
            exact_static_number,
            array_binding,
            module_assignment_expression,
            resolved_local_binding,
            returned_descriptor_binding,
            resolved_name,
            is_internal_array_iterator_binding,
            is_internal_array_step_binding,
            is_internal_iterator_temp,
            iterator_binding_source,
        };

        if self.try_store_identifier_value_via_isolated_indirect_eval_path(
            name,
            value_local,
            &state,
        )? {
            return Ok(());
        }

        if let Some(parameter_scope_arguments_local) =
            self.parameter_scope_arguments_local_for(name)
        {
            self.push_local_get(value_local);
            self.push_local_set(parameter_scope_arguments_local);
        }

        self.apply_identifier_store_shared_updates(value_local, &state)?;

        if let Some((resolved_name, local_index)) = state.resolved_local_binding.as_ref() {
            self.store_identifier_value_to_resolved_local(
                name,
                value_local,
                resolved_name,
                *local_index,
                &state,
            )?;
        } else if self
            .resolve_user_function_capture_hidden_name(name)
            .is_some()
        {
            self.store_identifier_value_to_capture_binding(name, value_local, &state)?;
        } else if let Some(global_index) = self.backend.global_binding_index(name) {
            self.store_identifier_value_to_declared_global(
                name,
                value_local,
                global_index,
                &state,
            )?;
        } else if self.resolve_eval_local_function_hidden_name(name).is_some() {
            self.store_identifier_value_to_eval_local_hidden(name, value_local, &state)?;
        } else if let Some(binding) = self.backend.implicit_global_binding(name) {
            self.store_identifier_value_to_implicit_global(name, value_local, binding, &state)?;
        } else {
            let binding = self.ensure_implicit_global_binding(name);
            self.store_identifier_value_to_implicit_global(name, value_local, binding, &state)?;
        }

        Ok(())
    }
}
