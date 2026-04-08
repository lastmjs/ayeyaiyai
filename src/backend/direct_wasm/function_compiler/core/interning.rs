use super::*;

impl DirectWasmCompiler {
    pub(in crate::backend::direct_wasm) fn intern_string(&mut self, bytes: Vec<u8>) -> (u32, u32) {
        self.state.intern_string(bytes)
    }
}

impl<'a> FunctionCompiler<'a> {
    pub(in crate::backend::direct_wasm) fn intern_string(&mut self, bytes: Vec<u8>) -> (u32, u32) {
        self.backend.intern_string(bytes)
    }
}
