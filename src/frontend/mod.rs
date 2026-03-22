mod early_errors;
mod lowering;
mod modules;
mod parse;

pub use modules::{bundle_module_entry, bundle_script_entry};
pub use parse::{
    parse, parse_module_goal, parse_module_goal_with_path, parse_script_goal, validate_script_goal,
};

#[cfg(test)]
mod parse_tests;
