use super::*;

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct PreparedGlobalProgramContext {
    pub(in crate::backend::direct_wasm) shared_global_binding_environment:
        Rc<SharedGlobalBindingEnvironment>,
    pub(in crate::backend::direct_wasm) global_static_semantics: Rc<GlobalStaticSemanticsSnapshot>,
}

impl PreparedGlobalProgramContext {
    pub(in crate::backend::direct_wasm) fn new(
        global_binding_environment: GlobalBindingEnvironment,
        global_static_semantics: GlobalStaticSemanticsSnapshot,
    ) -> Self {
        Self {
            shared_global_binding_environment: Rc::new(
                SharedGlobalBindingEnvironment::from_binding_environment(
                    &global_binding_environment,
                ),
            ),
            global_static_semantics: Rc::new(global_static_semantics),
        }
    }

    pub(in crate::backend::direct_wasm) fn required_shared_global_binding_environment(
        &self,
    ) -> &SharedGlobalBindingEnvironment {
        self.shared_global_binding_environment.as_ref()
    }

    pub(in crate::backend::direct_wasm) fn required_global_static_semantics(
        &self,
    ) -> &GlobalStaticSemanticsSnapshot {
        self.global_static_semantics.as_ref()
    }
}
