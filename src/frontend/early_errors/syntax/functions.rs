use super::super::*;
use super::{
    bindings::collect_pattern_binding_names,
    declarations::{BindingRestrictions, validate_pattern_syntax_with_restrictions},
    expressions::validate_expression_syntax,
    statements::{validate_statement_syntax, validate_statement_syntax_with_restrictions},
};

pub(crate) fn validate_function_syntax(
    function: &Function,
    file: &swc_common::SourceFile,
) -> Result<()> {
    validate_function_syntax_with_explicit_strictness(
        function,
        file,
        function_has_use_strict_directive(function),
    )
}

fn validate_function_syntax_with_explicit_strictness(
    function: &Function,
    file: &swc_common::SourceFile,
    strict: bool,
) -> Result<()> {
    let restrictions = BindingRestrictions {
        await_reserved: function.is_async,
        yield_reserved: function.is_generator,
    };
    ensure_parameter_names_are_valid(
        function.params.iter().map(|parameter| &parameter.pat),
        function
            .params
            .iter()
            .all(|parameter| matches!(parameter.pat, Pat::Ident(_))),
        strict,
    )?;
    for parameter in &function.params {
        validate_pattern_syntax_with_restrictions(&parameter.pat, file, restrictions)?;
    }
    if let Some(body) = &function.body {
        for statement in &body.stmts {
            validate_statement_syntax_with_restrictions(statement, file, restrictions)?;
        }
    }

    Ok(())
}

pub(super) fn ensure_parameter_names_are_valid<'a>(
    parameters: impl IntoIterator<Item = &'a Pat>,
    has_simple_parameter_list: bool,
    strict: bool,
) -> Result<()> {
    let mut seen = HashSet::new();
    let mut duplicate = None;

    for parameter in parameters {
        let mut names = Vec::new();
        collect_pattern_binding_names(parameter, &mut names)?;
        for name in names {
            if !seen.insert(name.clone()) && duplicate.is_none() {
                duplicate = Some(name);
            }
        }
    }

    if let Some(name) = duplicate {
        ensure!(
            has_simple_parameter_list && !strict,
            "duplicate parameter name `{name}`"
        );
    }

    Ok(())
}

pub(crate) fn validate_class_syntax(class: &Class, file: &swc_common::SourceFile) -> Result<()> {
    if let Some(super_class) = &class.super_class {
        validate_expression_syntax(super_class, file)?;
    }

    for member in &class.body {
        match member {
            ClassMember::Constructor(constructor) => {
                validate_constructor_syntax(constructor, file, true)?;
            }
            ClassMember::Method(method) => {
                validate_property_name_syntax(&method.key, file)?;
                validate_function_syntax_with_explicit_strictness(&method.function, file, true)?;
            }
            ClassMember::ClassProp(property) => {
                validate_property_name_syntax(&property.key, file)?;
                if let Some(value) = &property.value {
                    validate_expression_syntax(value, file)?;
                }
            }
            ClassMember::PrivateMethod(method) => {
                validate_function_syntax_with_explicit_strictness(&method.function, file, true)?;
            }
            ClassMember::PrivateProp(property) => {
                if let Some(value) = &property.value {
                    validate_expression_syntax(value, file)?;
                }
            }
            ClassMember::StaticBlock(block) => {
                for statement in &block.body.stmts {
                    validate_statement_syntax(statement, file)?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_constructor_syntax(
    constructor: &Constructor,
    file: &swc_common::SourceFile,
    strict: bool,
) -> Result<()> {
    ensure_parameter_names_are_valid(
        constructor.params.iter().filter_map(|parameter| match parameter {
            ParamOrTsParamProp::Param(parameter) => Some(&parameter.pat),
            ParamOrTsParamProp::TsParamProp(_) => None,
        }),
        constructor
            .params
            .iter()
            .all(|parameter| matches!(parameter, ParamOrTsParamProp::Param(parameter) if matches!(parameter.pat, Pat::Ident(_)))),
        strict,
    )?;
    for parameter in &constructor.params {
        match parameter {
            ParamOrTsParamProp::Param(parameter) => validate_pattern_syntax_with_restrictions(
                &parameter.pat,
                file,
                BindingRestrictions::default(),
            )?,
            ParamOrTsParamProp::TsParamProp(_) => {}
        }
    }
    if let Some(body) = &constructor.body {
        for statement in &body.stmts {
            validate_statement_syntax(statement, file)?;
        }
    }

    Ok(())
}

pub(super) fn validate_property_name_syntax(
    name: &PropName,
    file: &swc_common::SourceFile,
) -> Result<()> {
    if let PropName::Computed(computed) = name {
        validate_expression_syntax(&computed.expr, file)?;
    }

    Ok(())
}
