use super::super::*;
use super::bindings::binding_ident;

pub(crate) fn lower_parameters(
    lowerer: &mut Lowerer,
    function: &Function,
) -> Result<(Vec<Parameter>, Vec<Statement>)> {
    lower_parameter_patterns(
        lowerer,
        function.params.iter().map(|parameter| &parameter.pat),
    )
}

pub(crate) fn lower_constructor_parameters(
    lowerer: &mut Lowerer,
    constructor: &Constructor,
) -> Result<(Vec<Parameter>, Vec<Statement>, usize)> {
    let mut patterns = Vec::with_capacity(constructor.params.len());
    for parameter in &constructor.params {
        let ParamOrTsParamProp::Param(parameter) = parameter else {
            bail!("parameter properties are not supported yet")
        };
        patterns.push(&parameter.pat);
    }

    let (params, setup) = lower_parameter_patterns(lowerer, patterns.iter().copied())?;
    Ok((
        params,
        setup,
        expected_argument_count(patterns.iter().copied()),
    ))
}

pub(crate) fn lower_parameter_patterns<'a>(
    lowerer: &mut Lowerer,
    parameters: impl IntoIterator<Item = &'a Pat>,
) -> Result<(Vec<Parameter>, Vec<Statement>)> {
    let mut lowered_parameters = Vec::new();
    let mut setup = Vec::new();

    for parameter in parameters {
        let (lowered, mut lowered_setup) = lower_parameter(lowerer, parameter)?;
        lowered_parameters.push(lowered);
        setup.append(&mut lowered_setup);
    }

    Ok((lowered_parameters, setup))
}

pub(crate) fn lower_parameter(
    lowerer: &mut Lowerer,
    parameter: &Pat,
) -> Result<(Parameter, Vec<Statement>)> {
    match parameter {
        Pat::Ident(identifier) => Ok((
            Parameter {
                name: lowerer.resolve_binding_name(identifier.id.sym.as_ref()),
                default: None,
                rest: false,
            },
            Vec::new(),
        )),
        Pat::Assign(assign) => match &*assign.left {
            Pat::Ident(identifier) => Ok((
                Parameter {
                    name: lowerer.resolve_binding_name(identifier.id.sym.as_ref()),
                    default: Some(lowerer.lower_expression(&assign.right)?),
                    rest: false,
                },
                Vec::new(),
            )),
            pattern => {
                let temporary_name = lowerer.fresh_temporary_name("param");
                let mut setup = Vec::new();
                lowerer.lower_for_of_pattern_binding(
                    pattern,
                    Expression::Identifier(temporary_name.clone()),
                    ForOfPatternBindingKind::Lexical { mutable: true },
                    &mut setup,
                )?;
                Ok((
                    Parameter {
                        name: temporary_name,
                        default: Some(lowerer.lower_expression(&assign.right)?),
                        rest: false,
                    },
                    setup,
                ))
            }
        },
        Pat::Rest(rest) => {
            if let Ok(BindingIdent { id, .. }) = binding_ident(&rest.arg) {
                return Ok((
                    Parameter {
                        name: lowerer.resolve_binding_name(id.sym.as_ref()),
                        default: None,
                        rest: true,
                    },
                    Vec::new(),
                ));
            }

            let temporary_name = lowerer.fresh_temporary_name("rest");
            let mut setup = Vec::new();
            lowerer.lower_for_of_pattern_binding(
                &rest.arg,
                Expression::Identifier(temporary_name.clone()),
                ForOfPatternBindingKind::Lexical { mutable: true },
                &mut setup,
            )?;
            Ok((
                Parameter {
                    name: temporary_name,
                    default: None,
                    rest: true,
                },
                setup,
            ))
        }
        pattern => {
            let temporary_name = lowerer.fresh_temporary_name("param");
            let mut setup = Vec::new();
            lowerer.lower_for_of_pattern_binding(
                pattern,
                Expression::Identifier(temporary_name.clone()),
                ForOfPatternBindingKind::Lexical { mutable: true },
                &mut setup,
            )?;
            Ok((
                Parameter {
                    name: temporary_name,
                    default: None,
                    rest: false,
                },
                setup,
            ))
        }
    }
}

pub(crate) fn expected_argument_count<'a>(parameters: impl IntoIterator<Item = &'a Pat>) -> usize {
    let mut count = 0;
    for parameter in parameters {
        match parameter {
            Pat::Rest(_) | Pat::Assign(_) => break,
            _ => count += 1,
        }
    }
    count
}

pub(crate) fn function_has_simple_parameter_list(function: &Function) -> bool {
    function
        .params
        .iter()
        .all(|parameter| matches!(parameter.pat, Pat::Ident(_)))
}
