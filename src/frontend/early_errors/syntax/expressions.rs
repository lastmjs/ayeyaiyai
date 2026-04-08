use super::super::*;
use super::{
    declarations::{
        BindingRestrictions, is_await_like_identifier, is_yield_like_identifier,
        validate_pattern_syntax, validate_pattern_syntax_with_restrictions,
    },
    functions::{
        ensure_parameter_names_are_valid, validate_class_syntax, validate_function_syntax,
        validate_property_name_syntax,
    },
    statements::{validate_statement_syntax, validate_statement_syntax_with_restrictions},
};

fn validate_digit_sequence(
    digits: &str,
    raw: &str,
    valid_digit: impl Fn(u8) -> bool,
) -> Result<()> {
    ensure!(!digits.is_empty(), "invalid numeric literal `{raw}`");

    let mut saw_digit = false;
    let mut previous_was_separator = false;

    for byte in digits.bytes() {
        if byte == b'_' {
            ensure!(
                saw_digit && !previous_was_separator,
                "invalid numeric literal `{raw}`"
            );
            previous_was_separator = true;
            continue;
        }

        ensure!(valid_digit(byte), "invalid numeric literal `{raw}`");
        saw_digit = true;
        previous_was_separator = false;
    }

    ensure!(
        saw_digit && !previous_was_separator,
        "invalid numeric literal `{raw}`"
    );
    Ok(())
}

fn validate_based_integer_literal(
    digits: &str,
    raw: &str,
    valid_digit: impl Fn(u8) -> bool,
) -> Result<()> {
    validate_digit_sequence(digits, raw, valid_digit)
}

fn validate_decimal_integer_digits(digits: &str, raw: &str) -> Result<()> {
    validate_digit_sequence(digits, raw, |byte| byte.is_ascii_digit())?;
    if digits.contains('_') && digits.starts_with('0') && digits.len() > 1 {
        bail!("invalid numeric literal `{raw}`");
    }
    Ok(())
}

fn validate_decimal_literal(raw: &str) -> Result<()> {
    let (mantissa, exponent) = match raw.find(['e', 'E']) {
        Some(index) => (&raw[..index], Some(&raw[index + 1..])),
        None => (raw, None),
    };

    if let Some(dot_index) = mantissa.find('.') {
        let integer = &mantissa[..dot_index];
        let fraction = &mantissa[dot_index + 1..];
        ensure!(
            !integer.is_empty() || !fraction.is_empty(),
            "invalid numeric literal `{raw}`"
        );
        if !integer.is_empty() {
            validate_decimal_integer_digits(integer, raw)?;
        }
        if !fraction.is_empty() {
            validate_digit_sequence(fraction, raw, |byte| byte.is_ascii_digit())?;
        }
    } else {
        validate_decimal_integer_digits(mantissa, raw)?;
    }

    if let Some(exponent_digits) = exponent {
        let exponent_digits = exponent_digits
            .strip_prefix('+')
            .or_else(|| exponent_digits.strip_prefix('-'))
            .unwrap_or(exponent_digits);
        validate_digit_sequence(exponent_digits, raw, |byte| byte.is_ascii_digit())?;
    }

    Ok(())
}

fn validate_number_literal_syntax(number: &Number, file: &swc_common::SourceFile) -> Result<()> {
    let raw = number.raw.as_deref().map(str::to_owned).unwrap_or_else(|| {
        source_slice_for_span(file, number.span)
            .unwrap_or("")
            .to_string()
    });
    if !raw.contains('_') {
        return Ok(());
    }

    let normalized = raw.strip_suffix('n').unwrap_or(&raw);
    if let Some(digits) = normalized
        .strip_prefix("0b")
        .or_else(|| normalized.strip_prefix("0B"))
    {
        return validate_based_integer_literal(digits, &raw, |byte| matches!(byte, b'0' | b'1'));
    }
    if let Some(digits) = normalized
        .strip_prefix("0o")
        .or_else(|| normalized.strip_prefix("0O"))
    {
        return validate_based_integer_literal(digits, &raw, |byte| (b'0'..=b'7').contains(&byte));
    }
    if let Some(digits) = normalized
        .strip_prefix("0x")
        .or_else(|| normalized.strip_prefix("0X"))
    {
        return validate_based_integer_literal(digits, &raw, |byte| byte.is_ascii_hexdigit());
    }

    validate_decimal_literal(normalized)
}

pub(crate) fn validate_expression_syntax(
    expression: &Expr,
    file: &swc_common::SourceFile,
) -> Result<()> {
    validate_expression_syntax_with_restrictions(expression, file, BindingRestrictions::default())
}

pub(super) fn validate_expression_syntax_with_restrictions(
    expression: &Expr,
    file: &swc_common::SourceFile,
    restrictions: BindingRestrictions,
) -> Result<()> {
    match expression {
        Expr::Lit(Lit::Num(number)) => validate_number_literal_syntax(number, file)?,
        Expr::Ident(identifier) => {
            ensure!(
                !(restrictions.await_reserved && is_await_like_identifier(identifier.sym.as_ref())),
                "`await` cannot be used as an identifier in an async function"
            );
            ensure!(
                !(restrictions.yield_reserved && is_yield_like_identifier(identifier.sym.as_ref())),
                "`yield` cannot be used as an identifier in a generator function"
            );
        }
        Expr::Call(call) => {
            if let Callee::Expr(callee) = &call.callee {
                validate_expression_syntax_with_restrictions(callee, file, restrictions)?;
            }
            for argument in &call.args {
                validate_expression_syntax_with_restrictions(&argument.expr, file, restrictions)?;
            }
        }
        Expr::New(new_expression) => {
            validate_expression_syntax_with_restrictions(
                &new_expression.callee,
                file,
                restrictions,
            )?;
            for argument in new_expression.args.iter().flatten() {
                validate_expression_syntax_with_restrictions(&argument.expr, file, restrictions)?;
            }
        }
        Expr::Await(await_expression) => {
            validate_expression_syntax_with_restrictions(
                &await_expression.arg,
                file,
                restrictions,
            )?;
        }
        Expr::Yield(yield_expression) => {
            if let Some(argument) = &yield_expression.arg {
                validate_expression_syntax_with_restrictions(argument, file, restrictions)?;
            }
        }
        Expr::Paren(parenthesized) => {
            validate_expression_syntax_with_restrictions(&parenthesized.expr, file, restrictions)?
        }
        Expr::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_expression_syntax_with_restrictions(&element.expr, file, restrictions)?;
            }
        }
        Expr::Object(object) => {
            for property in &object.props {
                match property {
                    PropOrSpread::Spread(spread) => validate_expression_syntax_with_restrictions(
                        &spread.expr,
                        file,
                        restrictions,
                    )?,
                    PropOrSpread::Prop(property) => match &**property {
                        Prop::Shorthand(_) => {}
                        Prop::KeyValue(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            validate_expression_syntax_with_restrictions(
                                &property.value,
                                file,
                                restrictions,
                            )?;
                        }
                        Prop::Getter(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    validate_statement_syntax(statement, file)?;
                                }
                            }
                        }
                        Prop::Setter(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            validate_pattern_syntax(&property.param, file)?;
                            if let Some(body) = &property.body {
                                for statement in &body.stmts {
                                    validate_statement_syntax(statement, file)?;
                                }
                            }
                        }
                        Prop::Method(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            validate_function_syntax(&property.function, file)?;
                        }
                        Prop::Assign(property) => validate_expression_syntax_with_restrictions(
                            &property.value,
                            file,
                            restrictions,
                        )?,
                    },
                }
            }
        }
        Expr::Member(member) => {
            validate_expression_syntax_with_restrictions(&member.obj, file, restrictions)?;
            if let MemberProp::Computed(property) = &member.prop {
                validate_expression_syntax_with_restrictions(&property.expr, file, restrictions)?;
            }
        }
        Expr::Unary(unary) => {
            validate_expression_syntax_with_restrictions(&unary.arg, file, restrictions)?
        }
        Expr::Update(update) => {
            validate_expression_syntax_with_restrictions(&update.arg, file, restrictions)?
        }
        Expr::Bin(binary) => {
            validate_expression_syntax_with_restrictions(&binary.left, file, restrictions)?;
            validate_expression_syntax_with_restrictions(&binary.right, file, restrictions)?;
        }
        Expr::Assign(assignment) => {
            match &assignment.left {
                AssignTarget::Simple(SimpleAssignTarget::Member(member)) => {
                    validate_expression_syntax_with_restrictions(&member.obj, file, restrictions)?;
                    if let MemberProp::Computed(property) = &member.prop {
                        validate_expression_syntax_with_restrictions(
                            &property.expr,
                            file,
                            restrictions,
                        )?;
                    }
                }
                AssignTarget::Simple(_) | AssignTarget::Pat(_) => {}
            }
            validate_expression_syntax_with_restrictions(&assignment.right, file, restrictions)?;
        }
        Expr::Cond(conditional) => {
            validate_expression_syntax_with_restrictions(&conditional.test, file, restrictions)?;
            validate_expression_syntax_with_restrictions(&conditional.cons, file, restrictions)?;
            validate_expression_syntax_with_restrictions(&conditional.alt, file, restrictions)?;
        }
        Expr::Seq(sequence) => {
            for expression in &sequence.exprs {
                validate_expression_syntax_with_restrictions(expression, file, restrictions)?;
            }
        }
        Expr::Fn(function) => validate_function_syntax(&function.function, file)?,
        Expr::Arrow(arrow) => {
            let restrictions = BindingRestrictions {
                await_reserved: arrow.is_async,
                yield_reserved: false,
            };
            ensure_parameter_names_are_valid(
                arrow.params.iter(),
                arrow
                    .params
                    .iter()
                    .all(|parameter| matches!(parameter, Pat::Ident(_))),
                false,
            )?;
            for parameter in &arrow.params {
                validate_pattern_syntax_with_restrictions(parameter, file, restrictions)?;
            }
            match &*arrow.body {
                BlockStmtOrExpr::BlockStmt(block) => {
                    for statement in &block.stmts {
                        validate_statement_syntax_with_restrictions(statement, file, restrictions)?;
                    }
                }
                BlockStmtOrExpr::Expr(expression) => {
                    validate_expression_syntax_with_restrictions(expression, file, restrictions)?
                }
            }
        }
        Expr::Class(class) => validate_class_syntax(&class.class, file)?,
        Expr::Tpl(template) => {
            for expression in &template.exprs {
                validate_expression_syntax_with_restrictions(expression, file, restrictions)?;
            }
        }
        Expr::TaggedTpl(tagged) => {
            validate_expression_syntax_with_restrictions(&tagged.tag, file, restrictions)?;
            for expression in &tagged.tpl.exprs {
                validate_expression_syntax_with_restrictions(expression, file, restrictions)?;
            }
        }
        _ => {}
    }

    Ok(())
}
