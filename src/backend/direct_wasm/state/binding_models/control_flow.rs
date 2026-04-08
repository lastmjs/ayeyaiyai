use super::*;

pub(in crate::backend::direct_wasm) struct CompiledFunction {
    pub(in crate::backend::direct_wasm) local_count: u32,
    pub(in crate::backend::direct_wasm) instructions: Vec<u8>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct LoopContext {
    pub(in crate::backend::direct_wasm) break_target: usize,
    pub(in crate::backend::direct_wasm) continue_target: usize,
    pub(in crate::backend::direct_wasm) labels: Vec<String>,
    pub(in crate::backend::direct_wasm) assigned_bindings: HashSet<String>,
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct BreakContext {
    pub(in crate::backend::direct_wasm) break_target: usize,
    pub(in crate::backend::direct_wasm) labels: Vec<String>,
    pub(in crate::backend::direct_wasm) break_hook: Option<Expression>,
}

pub(in crate::backend::direct_wasm) struct MaterializationGuard<'a> {
    pub(in crate::backend::direct_wasm) active: &'a RefCell<HashSet<usize>>,
    pub(in crate::backend::direct_wasm) key: usize,
}

impl Drop for MaterializationGuard<'_> {
    fn drop(&mut self) {
        self.active.borrow_mut().remove(&self.key);
    }
}

#[derive(Clone)]
pub(in crate::backend::direct_wasm) struct TryContext {
    pub(in crate::backend::direct_wasm) catch_target: usize,
}
