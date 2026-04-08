use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(super) fn clear_member_function_bindings_for_name(&mut self, name: &str) {
        self.state.clear_member_bindings_for_name(name, true);
        if self.binding_name_is_global(name) {
            self.backend.clear_global_member_bindings_for_name(name);
        }
    }

    pub(super) fn clear_object_literal_member_bindings_for_name(&mut self, name: &str) {
        self.state.clear_member_bindings_for_name(name, false);
        if self.binding_name_is_global(name) {
            self.backend
                .clear_global_object_literal_member_bindings_for_name(name);
        }
    }
}
