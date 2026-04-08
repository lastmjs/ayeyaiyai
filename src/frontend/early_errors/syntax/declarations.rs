use super::super::*;
use super::{
    expressions::validate_expression_syntax_with_restrictions,
    functions::{validate_class_syntax, validate_function_syntax},
};

#[derive(Clone, Copy, Default)]
pub(super) struct BindingRestrictions {
    pub(super) await_reserved: bool,
    pub(super) yield_reserved: bool,
}

pub(super) fn is_await_like_identifier(name: &str) -> bool {
    name == "await" || name == "__ayy_await_ident"
}

pub(super) fn is_yield_like_identifier(name: &str) -> bool {
    name == "yield"
}

fn is_valid_identifier_part(character: char) -> bool {
    matches!(character, '\u{200C}' | '\u{200D}') || Ident::is_valid_continue(character)
}

fn decode_identifier_escape(raw: &str, characters: &mut std::str::Chars<'_>) -> Result<char> {
    ensure!(
        matches!(characters.next(), Some('u')),
        "invalid unicode escape in identifier `{raw}`"
    );

    if matches!(characters.clone().next(), Some('{')) {
        characters.next();
        let mut digits = String::new();
        let mut closed = false;
        for character in characters.by_ref() {
            if character == '}' {
                closed = true;
                break;
            }
            ensure!(
                character.is_ascii_hexdigit(),
                "invalid unicode escape in identifier `{raw}`"
            );
            digits.push(character);
        }
        ensure!(
            closed && !digits.is_empty(),
            "invalid unicode escape in identifier `{raw}`"
        );
        let code_point = u32::from_str_radix(&digits, 16)
            .context("unicode escape digits should be hexadecimal")?;
        return char::from_u32(code_point)
            .with_context(|| format!("invalid unicode escape in identifier `{raw}`"));
    }

    let mut digits = String::new();
    for _ in 0..4 {
        let Some(character) = characters.next() else {
            bail!("invalid unicode escape in identifier `{raw}`");
        };
        ensure!(
            character.is_ascii_hexdigit(),
            "invalid unicode escape in identifier `{raw}`"
        );
        digits.push(character);
    }
    let code_point =
        u32::from_str_radix(&digits, 16).context("unicode escape digits should be hexadecimal")?;
    char::from_u32(code_point)
        .with_context(|| format!("invalid unicode escape in identifier `{raw}`"))
}

fn validate_escaped_identifier_text(raw: &str) -> Result<()> {
    let mut decoded = Vec::new();
    let mut characters = raw.chars();

    while let Some(character) = characters.next() {
        decoded.push(if character == '\\' {
            decode_identifier_escape(raw, &mut characters)?
        } else {
            character
        });
    }

    let Some(first) = decoded.first().copied() else {
        bail!("invalid identifier `{raw}`");
    };
    ensure!(Ident::is_valid_start(first), "invalid identifier `{raw}`");
    ensure!(
        decoded
            .iter()
            .skip(1)
            .copied()
            .all(is_valid_identifier_part),
        "invalid identifier `{raw}`"
    );
    Ok(())
}

fn validate_binding_identifier_syntax(
    identifier: &BindingIdent,
    file: &swc_common::SourceFile,
    restrictions: BindingRestrictions,
) -> Result<()> {
    let raw = source_slice_for_span(file, identifier.id.span)?;
    if raw.contains('\\') {
        validate_escaped_identifier_text(raw)?;
        ensure!(
            !identifier.id.is_reserved(),
            "reserved word `{}` cannot be escaped in a binding identifier",
            identifier.id.sym
        );
    }
    ensure!(
        !(restrictions.await_reserved && is_await_like_identifier(identifier.id.sym.as_ref())),
        "`await` cannot be used as a binding identifier in an async function"
    );
    ensure!(
        !(restrictions.yield_reserved && is_yield_like_identifier(identifier.id.sym.as_ref())),
        "`yield` cannot be used as a binding identifier in a generator function"
    );

    Ok(())
}

pub(crate) fn validate_declaration_syntax(
    declaration: &Decl,
    file: &swc_common::SourceFile,
) -> Result<()> {
    match declaration {
        Decl::Fn(function) => validate_function_syntax(&function.function, file)?,
        Decl::Class(class) => validate_class_syntax(&class.class, file)?,
        Decl::Var(variable_declaration) => {
            validate_variable_declaration_syntax(variable_declaration, file)?;
        }
        _ => {}
    }

    Ok(())
}

pub(super) fn validate_variable_declaration_syntax(
    declaration: &swc_ecma_ast::VarDecl,
    file: &swc_common::SourceFile,
) -> Result<()> {
    validate_variable_declaration_syntax_with_restrictions(
        declaration,
        file,
        BindingRestrictions::default(),
    )
}

pub(super) fn validate_variable_declaration_syntax_with_restrictions(
    declaration: &swc_ecma_ast::VarDecl,
    file: &swc_common::SourceFile,
    restrictions: BindingRestrictions,
) -> Result<()> {
    for declarator in &declaration.decls {
        validate_pattern_syntax_with_restrictions(&declarator.name, file, restrictions)?;
        if let Some(initializer) = &declarator.init {
            validate_expression_syntax_with_restrictions(initializer, file, restrictions)?;
        }
    }

    Ok(())
}

pub(super) fn validate_for_head_syntax(
    head: &ForHead,
    file: &swc_common::SourceFile,
) -> Result<()> {
    validate_for_head_syntax_with_restrictions(head, file, BindingRestrictions::default())
}

pub(super) fn validate_for_head_syntax_with_restrictions(
    head: &ForHead,
    file: &swc_common::SourceFile,
    restrictions: BindingRestrictions,
) -> Result<()> {
    match head {
        ForHead::VarDecl(variable_declaration) => {
            validate_variable_declaration_syntax_with_restrictions(
                variable_declaration,
                file,
                restrictions,
            )?;
        }
        ForHead::Pat(pattern) => {
            validate_pattern_syntax_with_restrictions(pattern, file, restrictions)?
        }
        ForHead::UsingDecl(_) => {}
    }

    Ok(())
}

pub(super) fn validate_pattern_syntax(pattern: &Pat, file: &swc_common::SourceFile) -> Result<()> {
    validate_pattern_syntax_with_restrictions(pattern, file, BindingRestrictions::default())
}

pub(super) fn validate_pattern_syntax_with_restrictions(
    pattern: &Pat,
    file: &swc_common::SourceFile,
    restrictions: BindingRestrictions,
) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            validate_binding_identifier_syntax(identifier, file, restrictions)?
        }
        Pat::Assign(assign) => {
            validate_pattern_syntax_with_restrictions(&assign.left, file, restrictions)?;
            validate_expression_syntax_with_restrictions(&assign.right, file, restrictions)?;
        }
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_pattern_syntax_with_restrictions(element, file, restrictions)?;
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        validate_pattern_syntax_with_restrictions(
                            &property.value,
                            file,
                            restrictions,
                        )?;
                    }
                    ObjectPatProp::Assign(property) => {
                        let raw = source_slice_for_span(file, property.key.span)?;
                        if raw.contains('\\') {
                            validate_escaped_identifier_text(raw)?;
                            ensure!(
                                !property.key.is_reserved(),
                                "reserved word `{}` cannot be escaped in a binding identifier",
                                property.key.sym
                            );
                        }
                        ensure!(
                            !(restrictions.await_reserved
                                && is_await_like_identifier(property.key.sym.as_ref())),
                            "`await` cannot be used as a binding identifier in an async function"
                        );
                        ensure!(
                            !(restrictions.yield_reserved
                                && is_yield_like_identifier(property.key.sym.as_ref())),
                            "`yield` cannot be used as a binding identifier in a generator function"
                        );
                        if let Some(value) = &property.value {
                            validate_expression_syntax_with_restrictions(
                                value,
                                file,
                                restrictions,
                            )?;
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        validate_pattern_syntax_with_restrictions(&rest.arg, file, restrictions)?
                    }
                }
            }
        }
        Pat::Rest(rest) => {
            validate_pattern_syntax_with_restrictions(&rest.arg, file, restrictions)?
        }
        _ => {}
    }

    Ok(())
}
