use super::*;

#[derive(Default)]
pub(in crate::backend::direct_wasm) struct UserFunctionCatalog {
    pub(in crate::backend::direct_wasm) user_functions: Vec<UserFunction>,
    pub(in crate::backend::direct_wasm) registered_function_declarations: Vec<FunctionDeclaration>,
    pub(in crate::backend::direct_wasm) user_function_map: HashMap<String, UserFunction>,
}
