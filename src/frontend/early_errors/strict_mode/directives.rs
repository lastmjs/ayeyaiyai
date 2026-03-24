use super::super::*;

pub(super) fn is_strict_mode_restricted_identifier(name: &str) -> bool {
    matches!(name, "eval" | "arguments")
}

pub(crate) fn script_has_use_strict_directive(statements: &[Stmt]) -> bool {
    for statement in statements {
        let Stmt::Expr(ExprStmt { expr, .. }) = statement else {
            break;
        };

        let Expr::Lit(Lit::Str(string)) = &**expr else {
            break;
        };

        if is_unescaped_use_strict_directive(string) {
            return true;
        }
    }

    false
}

pub(crate) fn function_has_use_strict_directive(function: &Function) -> bool {
    let Some(body) = &function.body else {
        return false;
    };

    for statement in &body.stmts {
        let Stmt::Expr(ExprStmt { expr, .. }) = statement else {
            break;
        };

        let Expr::Lit(Lit::Str(string)) = &**expr else {
            break;
        };

        if is_unescaped_use_strict_directive(string) {
            return true;
        }
    }

    false
}

fn is_unescaped_use_strict_directive(string: &swc_ecma_ast::Str) -> bool {
    if string.value.as_str() != Some("use strict") {
        return false;
    }

    matches!(
        string.raw.as_ref().map(|raw| raw.as_str()),
        Some("\"use strict\"" | "'use strict'")
    )
}
