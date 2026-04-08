use super::*;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn update_local_array_iterator_binding_with_source(
        &mut self,
        name: &str,
        source: Option<IteratorSourceKind>,
    ) {
        let Some(source) = source else {
            self.state
                .speculation
                .static_semantics
                .clear_local_array_iterator_binding(name);
            return;
        };
        let index_local = self
            .resolve_local_array_iterator_binding_name(name)
            .and_then(|binding_name| {
                self.state
                    .speculation
                    .static_semantics
                    .local_array_iterator_binding(&binding_name)
            })
            .map(|binding| binding.index_local)
            .unwrap_or_else(|| self.allocate_temp_local());
        let static_index = match &source {
            IteratorSourceKind::StaticArray { length_local, .. }
                if length_local.is_none() || name.starts_with("__ayy_array_iter_") =>
            {
                Some(0)
            }
            IteratorSourceKind::SimpleGenerator { .. } => Some(0),
            IteratorSourceKind::AsyncYieldDelegateGenerator { .. } => Some(0),
            _ => None,
        };
        self.state
            .speculation
            .static_semantics
            .set_local_array_iterator_binding(
                name,
                ArrayIteratorBinding {
                    source,
                    index_local,
                    static_index,
                },
            );
        self.push_i32_const(0);
        self.push_local_set(index_local);
        self.state
            .speculation
            .static_semantics
            .set_local_kind(name, StaticValueKind::Object);
    }

    pub(in crate::backend::direct_wasm) fn update_local_array_iterator_binding(
        &mut self,
        name: &str,
        value: &Expression,
    ) {
        let source = self.resolve_local_array_iterator_source(value);
        self.update_local_array_iterator_binding_with_source(name, source);
    }
}
