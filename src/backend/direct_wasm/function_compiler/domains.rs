use super::*;

pub(in crate::backend::direct_wasm) struct BindingDomain<'b, 'a> {
    compiler: &'b mut FunctionCompiler<'a>,
}

impl<'b, 'a> BindingDomain<'b, 'a> {
    pub(in crate::backend::direct_wasm) fn register_statements(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        self.compiler.register_bindings(statements)
    }
}

pub(in crate::backend::direct_wasm) struct ControlFlowDomain<'b, 'a> {
    compiler: &'b mut FunctionCompiler<'a>,
}

impl<'b, 'a> ControlFlowDomain<'b, 'a> {
    pub(in crate::backend::direct_wasm) fn emit_direct_scope(
        &mut self,
        statements: &[Statement],
    ) -> DirectResult<()> {
        self.compiler
            .emit_statements_in_direct_lexical_scope(statements)
    }
}

pub(in crate::backend::direct_wasm) struct ObjectModelDomain<'b, 'a> {
    compiler: &'b mut FunctionCompiler<'a>,
}

impl<'b, 'a> ObjectModelDomain<'b, 'a> {
    pub(in crate::backend::direct_wasm) fn sync_statement_tracking_effects(
        &mut self,
        statement: &Statement,
    ) {
        self.compiler
            .sync_static_statement_tracking_effects(statement);
    }
}

pub(in crate::backend::direct_wasm) struct IteratorDomain<'b, 'a> {
    compiler: &'b FunctionCompiler<'a>,
}

impl<'b, 'a> IteratorDomain<'b, 'a> {
    pub(in crate::backend::direct_wasm) fn depends_on_active_loop_assignment(
        &self,
        expression: &Expression,
    ) -> bool {
        self.compiler
            .expression_depends_on_active_loop_assignment(expression)
    }
}

pub(in crate::backend::direct_wasm) struct ExceptionDomain<'b, 'a> {
    compiler: &'b mut FunctionCompiler<'a>,
}

impl<'b, 'a> ExceptionDomain<'b, 'a> {
    pub(in crate::backend::direct_wasm) fn clear_throw_state(&mut self) {
        self.compiler.clear_local_throw_state();
        self.compiler.clear_global_throw_state();
    }
}

#[allow(dead_code)]
pub(in crate::backend::direct_wasm) struct AsyncGeneratorDomain<'b, 'a> {
    compiler: &'b FunctionCompiler<'a>,
}

#[allow(dead_code)]
impl<'b, 'a> AsyncGeneratorDomain<'b, 'a> {
    pub(in crate::backend::direct_wasm) fn current_function_is_async_generator(&self) -> bool {
        self.compiler
            .current_user_function()
            .is_some_and(|user_function| user_function.is_async() && user_function.is_generator())
    }
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn bindings_domain(&mut self) -> BindingDomain<'_, 'a> {
        BindingDomain { compiler: self }
    }

    pub(in crate::backend::direct_wasm) fn control_flow_domain(
        &mut self,
    ) -> ControlFlowDomain<'_, 'a> {
        ControlFlowDomain { compiler: self }
    }

    pub(in crate::backend::direct_wasm) fn object_model_domain(
        &mut self,
    ) -> ObjectModelDomain<'_, 'a> {
        ObjectModelDomain { compiler: self }
    }

    pub(in crate::backend::direct_wasm) fn iterator_domain(&self) -> IteratorDomain<'_, 'a> {
        IteratorDomain { compiler: self }
    }

    pub(in crate::backend::direct_wasm) fn exception_domain(&mut self) -> ExceptionDomain<'_, 'a> {
        ExceptionDomain { compiler: self }
    }

    #[allow(dead_code)]
    pub(in crate::backend::direct_wasm) fn async_generator_domain(
        &self,
    ) -> AsyncGeneratorDomain<'_, 'a> {
        AsyncGeneratorDomain { compiler: self }
    }

    pub(in crate::backend::direct_wasm) fn pop_scoped_lexical_bindings(
        &mut self,
        names: &[String],
    ) {
        for name in names.iter().rev() {
            self.state.pop_scoped_lexical_binding(name);
        }
    }
}
