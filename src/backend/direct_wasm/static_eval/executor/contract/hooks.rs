use crate::backend::direct_wasm::Expression;
use crate::backend::direct_wasm::StaticObjectBindingLookupEnvironment;

use super::StaticExecutorContext;

pub(in crate::backend::direct_wasm) trait StaticExpressionHooks:
    StaticExecutorContext
{
    fn lookup_binding_value(
        &self,
        name: &str,
        environment: &Self::Environment,
    ) -> Option<Expression> {
        environment.binding(name).cloned()
    }

    fn evaluate_special_expression(
        &self,
        _expression: &Expression,
        _environment: &mut Self::Environment,
    ) -> Option<Expression> {
        None
    }
}
