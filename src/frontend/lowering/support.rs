mod analysis;
mod async_lowering;
mod bindings;
mod descriptors;
mod operators;
mod parameters;

pub(crate) use self::analysis::{
    assert_throws_call, console_log_arguments, parse_bigint_literal, pattern_name_hint,
    static_member_property_name, template_quasi_text,
};
pub(crate) use self::async_lowering::asyncify_statements;
pub(crate) use self::bindings::{
    collect_direct_statement_lexical_bindings, collect_for_of_binding_names,
    collect_for_per_iteration_bindings, collect_function_scope_binding_names,
    collect_parameter_binding_names, collect_switch_bindings,
};
pub(crate) use self::descriptors::{
    data_property_descriptor, define_property_statement, getter_property_descriptor,
    setter_property_descriptor,
};
pub(crate) use self::operators::{
    lower_binary_operator, lower_function_kind, lower_unary_operator, lower_update_operator,
};
pub(crate) use self::parameters::{
    expected_argument_count, function_has_simple_parameter_list, lower_constructor_parameters,
    lower_parameter, lower_parameter_patterns, lower_parameters,
};
