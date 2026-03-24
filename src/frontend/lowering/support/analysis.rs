use super::super::*;

pub(crate) fn parse_bigint_literal(value: &str) -> Result<String> {
    Ok(value.to_string())
}

pub(crate) fn template_quasi_text(element: &swc_ecma_ast::TplElement) -> Result<String> {
    if let Some(cooked) = &element.cooked {
        Ok(cooked.to_string_lossy().into_owned())
    } else {
        Ok(element.raw.to_string())
    }
}

pub(crate) fn pattern_name_hint(pattern: &Pat) -> Option<&str> {
    match pattern {
        Pat::Ident(identifier) => Some(identifier.id.sym.as_ref()),
        _ => None,
    }
}

pub(crate) fn static_member_property_name(property: &MemberProp) -> Option<String> {
    match property {
        MemberProp::Ident(identifier) => Some(identifier.sym.to_string()),
        MemberProp::Computed(computed) => match computed.expr.as_ref() {
            Expr::Lit(Lit::Str(string)) => Some(string.value.to_string_lossy().into_owned()),
            _ => None,
        },
        MemberProp::PrivateName(_) => None,
    }
}

pub(crate) fn console_log_arguments(expression: &Expr) -> Option<&[swc_ecma_ast::ExprOrSpread]> {
    let Expr::Call(call) = expression else {
        return None;
    };

    let Callee::Expr(callee) = &call.callee else {
        return None;
    };

    let Expr::Member(member) = &**callee else {
        return None;
    };

    let Expr::Ident(object) = &*member.obj else {
        return None;
    };

    if object.sym != *"console" {
        return None;
    }

    match &member.prop {
        MemberProp::Ident(identifier) if identifier.sym == *"log" => Some(&call.args),
        _ => None,
    }
}

pub(crate) fn assert_throws_call(expression: &Expr) -> Option<&swc_ecma_ast::CallExpr> {
    let Expr::Call(call) = expression else {
        return None;
    };

    let Callee::Expr(callee) = &call.callee else {
        return None;
    };

    let Expr::Ident(identifier) = &**callee else {
        return None;
    };

    (identifier.sym == "__ayyAssertThrows").then_some(call)
}
