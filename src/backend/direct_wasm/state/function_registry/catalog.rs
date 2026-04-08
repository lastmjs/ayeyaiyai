#[path = "catalog/mutations.rs"]
mod mutations;
#[path = "catalog/queries.rs"]
mod queries;
#[path = "catalog/types.rs"]
mod types;

use super::*;

pub(in crate::backend::direct_wasm) use types::UserFunctionCatalog;
