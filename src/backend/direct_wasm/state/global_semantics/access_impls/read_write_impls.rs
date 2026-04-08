use super::super::super::super::*;
use super::super::access_trait::{
    GlobalStaticSemanticsReadServices, GlobalStaticSemanticsWriteServices,
};
use super::super::*;

impl GlobalStaticSemanticsReadServices for GlobalSemanticState {
    fn names(&self) -> &GlobalNameService {
        &self.names
    }

    fn values(&self) -> &GlobalValueService {
        &self.values
    }

    fn functions(&self) -> &GlobalFunctionService {
        &self.functions
    }

    fn members(&self) -> &GlobalMemberService {
        &self.members
    }
}

impl GlobalStaticSemanticsWriteServices for GlobalSemanticState {
    fn clear_global_static_binding_metadata(&mut self, name: &str) {
        self.values.clear_value_binding(name);
        self.values.sync_array_binding(name, None);
        self.values.sync_object_binding(name, None);
        self.values.sync_arguments_binding(name, None);
        self.functions.clear_function_binding(name);
        self.names.clear_kind(name);
    }

    fn clear_global_binding_state(&mut self, name: &str) {
        self.clear_global_static_binding_metadata(name);
        self.values.clear_proxy_binding(name);
        self.values.clear_prototype_object_binding(name);
        self.values.clear_property_descriptor(name);
        self.functions.clear_specialized_function_value(name);
    }
}

impl GlobalStaticSemanticsReadServices for GlobalStaticSemanticsSnapshot {
    fn names(&self) -> &GlobalNameService {
        &self.names
    }

    fn values(&self) -> &GlobalValueService {
        &self.values
    }

    fn functions(&self) -> &GlobalFunctionService {
        &self.functions
    }

    fn members(&self) -> &GlobalMemberService {
        &self.members
    }
}

impl GlobalStaticSemanticsWriteServices for GlobalStaticSemanticsSnapshot {
    fn clear_global_static_binding_metadata(&mut self, name: &str) {
        self.values.clear_value_binding(name);
        self.values.sync_array_binding(name, None);
        self.values.sync_object_binding(name, None);
        self.values.sync_arguments_binding(name, None);
        self.functions.clear_function_binding(name);
        self.names.clear_kind(name);
    }

    fn clear_global_binding_state(&mut self, name: &str) {
        self.clear_global_static_binding_metadata(name);
        self.values.clear_proxy_binding(name);
        self.values.clear_prototype_object_binding(name);
        self.values.clear_property_descriptor(name);
        self.functions.clear_specialized_function_value(name);
    }
}
