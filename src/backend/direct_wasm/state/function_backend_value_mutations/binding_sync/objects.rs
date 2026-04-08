use super::*;

impl<'a> FunctionCompilerBackend<'a> {
    pub(in crate::backend::direct_wasm) fn sync_global_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_object_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_proxy_binding(
        &mut self,
        name: &str,
        binding: Option<ProxyValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_proxy_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_prototype_object_binding(
        &mut self,
        name: &str,
        binding: Option<ObjectValueBinding>,
    ) {
        self.global_semantics
            .values
            .sync_prototype_object_binding(name, binding);
    }

    pub(in crate::backend::direct_wasm) fn sync_global_object_prototype_expression(
        &mut self,
        name: &str,
        prototype: Option<Expression>,
    ) {
        self.global_semantics
            .values
            .sync_object_prototype_expression(name, prototype);
    }
}
