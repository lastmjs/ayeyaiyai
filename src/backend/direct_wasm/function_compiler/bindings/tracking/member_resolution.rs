use super::*;

thread_local! {
    static MEMBER_FUNCTION_BINDING_RESOLUTION_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct MemberFunctionBindingResolutionGuard;

impl MemberFunctionBindingResolutionGuard {
    fn enter(object: &Expression, property: &Expression) -> Self {
        MEMBER_FUNCTION_BINDING_RESOLUTION_DEPTH.with(|depth| {
            let next = depth.get() + 1;
            if next > 256 {
                panic!(
                    "member function binding resolution recursion overflow: object={object:?}, property={property:?}"
                );
            }
            depth.set(next);
        });
        Self
    }
}

impl Drop for MemberFunctionBindingResolutionGuard {
    fn drop(&mut self) {
        MEMBER_FUNCTION_BINDING_RESOLUTION_DEPTH
            .with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

mod accessor_bindings;
mod binding_entries;
mod function_bindings;
mod iterator_reads;
mod proxy_bindings;
mod scope_helpers;
