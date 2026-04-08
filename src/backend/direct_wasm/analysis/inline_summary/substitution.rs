#[path = "substitution/binding_rewrite.rs"]
mod binding_rewrite;
#[path = "substitution/effect_free.rs"]
mod effect_free;
#[path = "substitution/matching.rs"]
mod matching;

pub(in crate::backend::direct_wasm) use self::{
    binding_rewrite::{
        rewrite_inline_function_summary_bindings, substitute_inline_summary_bindings,
    },
    effect_free::inline_summary_side_effect_free_expression,
    matching::static_expression_matches,
};
