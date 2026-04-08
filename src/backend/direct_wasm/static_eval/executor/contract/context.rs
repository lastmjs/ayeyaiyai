use crate::backend::direct_wasm::{
    StaticBindingEnvironment, StaticIdentifierBindingEnvironment, StaticIdentifierMaterializer,
    StaticLocalBindingEnvironment, StaticMutableObjectBindingEnvironment,
    StaticTransactionalEnvironment,
};

pub(in crate::backend::direct_wasm) trait StaticExecutorContext:
    StaticIdentifierMaterializer
{
    type Environment: StaticBindingEnvironment
        + StaticLocalBindingEnvironment
        + StaticMutableObjectBindingEnvironment
        + StaticIdentifierBindingEnvironment
        + StaticTransactionalEnvironment;
}
