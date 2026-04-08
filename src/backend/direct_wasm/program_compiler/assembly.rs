use super::*;
use crate::backend::direct_wasm::encoding::required_memory_pages;

pub(in crate::backend::direct_wasm) struct ModuleAssemblyInputs {
    pub(in crate::backend::direct_wasm) compiled_start: CompiledFunction,
    pub(in crate::backend::direct_wasm) compiled_functions: Vec<CompiledFunction>,
    pub(in crate::backend::direct_wasm) user_type_arities: Vec<u32>,
    pub(in crate::backend::direct_wasm) user_functions: Vec<UserFunction>,
    pub(in crate::backend::direct_wasm) string_data: Vec<(u32, Vec<u8>)>,
    pub(in crate::backend::direct_wasm) next_data_offset: u32,
    pub(in crate::backend::direct_wasm) global_binding_count: u32,
    pub(in crate::backend::direct_wasm) implicit_global_binding_count: u32,
    pub(in crate::backend::direct_wasm) runtime_prototype_binding_count: u32,
    pub(in crate::backend::direct_wasm) int_min_ptr: u32,
    pub(in crate::backend::direct_wasm) int_min_len: u32,
}

impl ModuleAssemblyInputs {
    pub(in crate::backend::direct_wasm) fn assemble(self) -> Vec<u8> {
        let initial_memory_pages = required_memory_pages(self.next_data_offset);

        let mut module = Vec::from(WASM_MAGIC_AND_VERSION);
        push_section(&mut module, 1, encode_type_section(&self.user_type_arities));
        push_section(&mut module, 2, encode_import_section());
        push_section(
            &mut module,
            3,
            encode_function_section(&self.user_functions),
        );
        push_section(&mut module, 5, encode_memory_section(initial_memory_pages));
        push_section(
            &mut module,
            6,
            encode_global_section(
                self.global_binding_count
                    + self.implicit_global_binding_count * 2
                    + self.runtime_prototype_binding_count,
            ),
        );
        push_section(&mut module, 7, encode_export_section());
        push_section(
            &mut module,
            10,
            encode_code_section(
                self.compiled_start,
                self.compiled_functions,
                self.int_min_ptr,
                self.int_min_len,
            ),
        );
        push_section(&mut module, 11, encode_data_section(&self.string_data));
        module
    }
}

impl EmittedBackendProgram {
    pub(in crate::backend::direct_wasm) fn assemble(self) -> Vec<u8> {
        ModuleAssemblyInputs {
            compiled_start: self.compiled_start,
            compiled_functions: self.compiled_functions,
            user_type_arities: self.module_layout.user_type_arities,
            user_functions: self.module_layout.user_functions,
            string_data: self.artifacts.string_data,
            next_data_offset: self.artifacts.next_data_offset,
            global_binding_count: self.module_layout.global_binding_count,
            implicit_global_binding_count: self.module_layout.implicit_global_binding_count,
            runtime_prototype_binding_count: self.module_layout.runtime_prototype_binding_count,
            int_min_ptr: self.artifacts.int_min_ptr,
            int_min_len: self.artifacts.int_min_len,
        }
        .assemble()
    }
}
