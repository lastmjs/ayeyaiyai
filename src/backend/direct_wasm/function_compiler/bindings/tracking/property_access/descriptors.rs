use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn binding_name_is_global(&self, name: &str) -> bool {
        self.state.speculation.execution_context.top_level_function
            && self.global_has_binding(name)
            && !self.state.runtime.locals.bindings.contains_key(name)
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

    pub(in crate::backend::direct_wasm) fn descriptor_expression_has_named_field(
        &self,
        descriptor: &Expression,
        descriptor_name: &str,
    ) -> bool {
        let Expression::Object(entries) = descriptor else {
            return false;
        };
        entries.iter().any(|entry| match entry {
            crate::ir::hir::ObjectEntry::Data { key, .. }
            | crate::ir::hir::ObjectEntry::Getter { key, .. }
            | crate::ir::hir::ObjectEntry::Setter { key, .. } => {
                matches!(key, Expression::String(name) if name == descriptor_name)
            }
            crate::ir::hir::ObjectEntry::Spread(_) => false,
        })
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
