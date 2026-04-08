use super::*;

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct ModuleArtifactsState {
    pub(in crate::backend::direct_wasm) string_data: Vec<(u32, Vec<u8>)>,
    pub(in crate::backend::direct_wasm) interned_strings: HashMap<Vec<u8>, (u32, u32)>,
    pub(in crate::backend::direct_wasm) next_data_offset: u32,
}

impl ModuleArtifactsState {
    pub(in crate::backend::direct_wasm) fn reset_for_program(&mut self) {
        self.string_data.clear();
        self.interned_strings.clear();
        self.next_data_offset = DATA_START_OFFSET;
    }

    pub(in crate::backend::direct_wasm) fn intern_string(&mut self, bytes: Vec<u8>) -> (u32, u32) {
        if let Some(existing) = self.interned_strings.get(&bytes) {
            return *existing;
        }

        let offset = self.next_data_offset;
        let len = bytes.len() as u32;
        self.next_data_offset += len;
        self.string_data.push((offset, bytes.clone()));
        self.interned_strings.insert(bytes, (offset, len));
        (offset, len)
    }

    pub(in crate::backend::direct_wasm) fn snapshot_data(&self) -> (Vec<(u32, Vec<u8>)>, u32) {
        (self.string_data.clone(), self.next_data_offset)
    }
}

impl CompilerState {
    pub(in crate::backend::direct_wasm) fn intern_string(&mut self, bytes: Vec<u8>) -> (u32, u32) {
        self.module_artifacts.intern_string(bytes)
    }

    pub(in crate::backend::direct_wasm) fn snapshot_module_data(
        &self,
    ) -> (Vec<(u32, Vec<u8>)>, u32) {
        self.module_artifacts.snapshot_data()
    }
}
