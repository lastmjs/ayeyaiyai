use super::*;

thread_local! {
    static FUNCTION_BINDING_RESOLUTION_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) struct FunctionBindingResolutionGuard;

impl FunctionBindingResolutionGuard {
    pub(super) fn enter(expression: &Expression, current_function_name: Option<&str>) -> Self {
        FUNCTION_BINDING_RESOLUTION_DEPTH.with(|depth| {
            let next = depth.get() + 1;
            if next > 256 {
                panic!(
                    "function binding resolution recursion overflow: current_function={current_function_name:?}, expression={expression:?}"
                );
            }
            depth.set(next);
        });
        Self
    }
}

impl Drop for FunctionBindingResolutionGuard {
    fn drop(&mut self) {
        FUNCTION_BINDING_RESOLUTION_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}
