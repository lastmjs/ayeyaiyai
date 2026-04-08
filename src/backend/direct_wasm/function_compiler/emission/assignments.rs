use super::*;
mod coercions;
mod function_outcomes;
mod primitive_effects;
#[path = "assignments/primitive_resolution.rs"]
mod primitive_resolution;
mod property_keys;
#[path = "assignments/special_assignments.rs"]
mod special_assignments;
mod static_builtin_eval;

impl<'a> FunctionCompiler<'a> {
    fn escape_static_json_string(text: &str) -> String {
        let mut escaped = String::with_capacity(text.len() + 2);
        escaped.push('"');
        for character in text.chars() {
            match character {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\u{08}' => escaped.push_str("\\b"),
                '\u{0C}' => escaped.push_str("\\f"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                character if character <= '\u{1F}' => {
                    use std::fmt::Write;
                    let _ = write!(escaped, "\\u{:04x}", character as u32);
                }
                character => escaped.push(character),
            }
        }
        escaped.push('"');
        escaped
    }
}
