use super::*;

pub(in crate::backend::direct_wasm) enum StaticStatementControl {
    Continue,
    Return(Expression),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::direct_wasm) enum StaticFunctionEffectMode {
    Commit,
    Discard,
}

pub(in crate::backend::direct_wasm) trait StaticFunctionExecutionEnvironment {
    fn clear_function_locals(&mut self);
}

pub(in crate::backend::direct_wasm) trait StaticBindingEnvironment {
    fn assign_binding_value(&mut self, name: String, value: Expression) -> Expression;

    fn sync_object_binding(&mut self, name: &str, binding: Option<ObjectValueBinding>);
}

pub(in crate::backend::direct_wasm) trait StaticLocalBindingEnvironment {
    fn set_local_binding(&mut self, name: String, value: Expression) -> Expression;
}

pub(in crate::backend::direct_wasm) trait StaticObjectBindingLookupEnvironment {
    fn binding(&self, name: &str) -> Option<&Expression>;

    fn contains_object_binding(&self, name: &str) -> bool;
}

pub(in crate::backend::direct_wasm) trait StaticMutableObjectBindingEnvironment:
    StaticObjectBindingLookupEnvironment
{
    fn object_binding_mut(&mut self, name: &str) -> Option<&mut ObjectValueBinding>;

    fn set_object_binding(&mut self, name: String, binding: ObjectValueBinding);
}

pub(in crate::backend::direct_wasm) trait StaticObjectBindingEnvironment:
    StaticObjectBindingLookupEnvironment
{
    fn object_binding(&self, name: &str) -> Option<&ObjectValueBinding>;
}

pub(in crate::backend::direct_wasm) trait StaticIdentifierBindingEnvironment:
    StaticObjectBindingEnvironment
{
    fn local_binding(&self, name: &str) -> Option<&Expression>;

    fn global_value_binding(&self, name: &str) -> Option<&Expression>;
}

pub(in crate::backend::direct_wasm) trait StaticTransactionalEnvironment:
    StaticFunctionExecutionEnvironment + Sized
{
    fn fork_environment(&self) -> Self;

    fn commit_environment(&mut self, environment: Self);
}
