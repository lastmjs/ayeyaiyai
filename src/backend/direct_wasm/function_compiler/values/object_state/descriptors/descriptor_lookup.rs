use super::*;

#[path = "descriptor_lookup/arguments.rs"]
mod arguments;
#[path = "descriptor_lookup/call_lookup.rs"]
mod call_lookup;
#[path = "descriptor_lookup/function_properties.rs"]
mod function_properties;
#[path = "descriptor_lookup/identifier_lookup.rs"]
mod identifier_lookup;
#[path = "descriptor_lookup/object_properties.rs"]
mod object_properties;
#[path = "descriptor_lookup/state_lookup.rs"]
mod state_lookup;
#[path = "descriptor_lookup/top_level.rs"]
mod top_level;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn resolve_descriptor_binding_from_expression(
        &self,
        expression: &Expression,
    ) -> Option<PropertyDescriptorBinding> {
        match expression {
            Expression::Identifier(name) => self.resolve_identifier_descriptor_binding(name),
            Expression::Call { callee, arguments } => {
                self.resolve_call_descriptor_binding(callee, arguments)
            }
            _ => None,
        }
    }
}
