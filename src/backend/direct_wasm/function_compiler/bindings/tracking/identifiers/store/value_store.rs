use super::*;

struct PreparedIdentifierValueStore {
    canonical_value_expression: Expression,
    tracked_value_expression: Expression,
    descriptor_binding_expression: Expression,
    tracked_object_expression: Expression,
    call_source_snapshot_expression: Option<Expression>,
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
}

mod context;
mod store_paths;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_store_identifier_value_local(
        &mut self,
        name: &str,
        value_expression: &Expression,
        value_local: u32,
    ) -> DirectResult<()> {
        self.state
            .runtime
            .locals
            .deleted_builtin_identifiers
            .remove(name);
        let prepared = self.prepare_identifier_value_store(name, value_expression);
        self.store_prepared_identifier_value_local(name, value_local, prepared)
    }
}
