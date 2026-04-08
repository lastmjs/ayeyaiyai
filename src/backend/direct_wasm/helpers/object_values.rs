use super::*;

pub(in crate::backend::direct_wasm) fn empty_object_value_binding() -> ObjectValueBinding {
    ObjectValueBinding {
        string_properties: Vec::new(),
        symbol_properties: Vec::new(),
        non_enumerable_string_properties: Vec::new(),
    }
}
