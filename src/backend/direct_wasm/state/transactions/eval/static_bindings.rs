use super::super::super::*;
use super::super::function::FunctionStaticBindingMetadataTransaction;
use super::super::global::GlobalStaticSemanticsTransaction;

pub(in crate::backend::direct_wasm) struct StaticBindingMetadataTransaction {
    pub(in crate::backend::direct_wasm) global_semantics: GlobalStaticSemanticsTransaction,
    pub(in crate::backend::direct_wasm) function_semantics:
        FunctionStaticBindingMetadataTransaction,
}

impl StaticBindingMetadataTransaction {
    pub(in crate::backend::direct_wasm) fn capture(
        compiler: &FunctionCompiler<'_>,
    ) -> StaticBindingMetadataTransaction {
        StaticBindingMetadataTransaction {
            global_semantics: compiler.backend.begin_global_static_semantics_transaction(),
            function_semantics: FunctionStaticBindingMetadataTransaction::capture(&compiler.state),
        }
    }

    pub(in crate::backend::direct_wasm) fn restore(self, compiler: &mut FunctionCompiler<'_>) {
        compiler
            .backend
            .restore_global_static_semantics_transaction(self.global_semantics);
        self.function_semantics.restore(&mut compiler.state);
    }
}
