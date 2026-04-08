#[path = "members_mutations.rs"]
mod mutations;
#[path = "members_queries.rs"]
mod queries;
#[path = "members_types.rs"]
mod types;

pub(in crate::backend::direct_wasm) use types::GlobalMemberService;
