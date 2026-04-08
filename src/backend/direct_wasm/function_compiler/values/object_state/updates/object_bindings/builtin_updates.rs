use super::super::super::*;

#[path = "builtin_updates/define_property.rs"]
mod define_property;
#[path = "builtin_updates/set_prototype.rs"]
mod set_prototype;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn apply_builtin_object_binding_updates(
        &mut self,
        expression: &Expression,
    ) {
        let Expression::Call { callee, arguments } = expression else {
            return;
        };
        let Expression::Member { object, property } = callee.as_ref() else {
            return;
        };
        if !matches!(object.as_ref(), Expression::Identifier(name) if name == "Object") {
            return;
        }
        if matches!(property.as_ref(), Expression::String(name) if name == "setPrototypeOf") {
            self.apply_object_set_prototype_of_update(arguments);
            return;
        }
        if matches!(property.as_ref(), Expression::String(name) if name == "defineProperty") {
            self.apply_object_define_property_update(arguments);
        }
    }
}
