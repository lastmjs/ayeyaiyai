use super::*;

#[path = "runtime_sources/array_like_updates.rs"]
mod array_like_updates;
#[path = "runtime_sources/generator_updates.rs"]
mod generator_updates;
#[path = "runtime_sources/static_outcomes.rs"]
mod static_outcomes;

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn emit_runtime_iterator_step_source_update(
        &mut self,
        iterator_binding: &mut ArrayIteratorBinding,
        current_static_index: Option<usize>,
        current_index_local: u32,
        sent_value: &Expression,
        done_local: u32,
        value_local: u32,
    ) {
        match &iterator_binding.source {
            IteratorSourceKind::StaticArray { .. } => self
                .update_runtime_iterator_step_static_array(
                    iterator_binding,
                    current_index_local,
                    done_local,
                    value_local,
                ),
            IteratorSourceKind::SimpleGenerator { .. } => self
                .update_runtime_iterator_step_simple_generator(
                    iterator_binding,
                    current_static_index,
                    current_index_local,
                    sent_value,
                    done_local,
                    value_local,
                ),
            IteratorSourceKind::AsyncYieldDelegateGenerator { .. } => {
                self.update_runtime_iterator_step_async_delegate(
                    iterator_binding,
                    done_local,
                    value_local,
                );
            }
            IteratorSourceKind::TypedArrayView { .. } => self
                .update_runtime_iterator_step_typed_array_view(
                    iterator_binding,
                    current_index_local,
                    done_local,
                    value_local,
                ),
            IteratorSourceKind::DirectArguments { .. } => self
                .update_runtime_iterator_step_direct_arguments(
                    iterator_binding,
                    current_index_local,
                    done_local,
                    value_local,
                ),
        }
    }
}
