use super::*;

#[path = "dispatch/call_entry.rs"]
mod call_entry;
mod dynamic_calls;
mod expression_calls;
#[path = "dispatch/function_prototype.rs"]
mod function_prototype;
#[path = "dispatch/function_this.rs"]
mod function_this;
#[path = "dispatch/member_getters.rs"]
mod member_getters;
#[path = "dispatch/prepared_calls.rs"]
mod prepared_calls;
#[path = "dispatch/snapshot_this.rs"]
mod snapshot_this;
