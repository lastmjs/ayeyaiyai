use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn update_user_function_home_object_binding(
        &mut self,
        binding: LocalFunctionBinding,
        home_object_name: &str,
    ) {
        let LocalFunctionBinding::User(function_name) = binding else {
            return;
        };
        self.set_user_function_home_object_binding(&function_name, home_object_name);
    }

    pub(in crate::backend::direct_wasm) fn update_global_object_literal_home_bindings(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let Expression::Object(entries) = value else {
            return;
        };
        for entry in entries {
            let binding = match entry {
                crate::ir::hir::ObjectEntry::Data { value, .. } => {
                    self.infer_global_function_binding(value)
                }
                crate::ir::hir::ObjectEntry::Getter { getter, .. } => {
                    self.infer_global_function_binding(getter)
                }
                crate::ir::hir::ObjectEntry::Setter { setter, .. } => {
                    self.infer_global_function_binding(setter)
                }
                crate::ir::hir::ObjectEntry::Spread(_) => None,
            };
            if let Some(binding) = binding {
                self.update_user_function_home_object_binding(binding, name);
            }
        }
    }
}
