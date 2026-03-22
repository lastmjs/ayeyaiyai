use super::*;

pub(super) fn validate_strict_mode_early_errors_in_module_items(
    items: &[ModuleItem],
    strict: bool,
) -> Result<()> {
    for item in items {
        match item {
            ModuleItem::Stmt(statement) => {
                validate_strict_mode_early_errors_in_statement(statement, strict)?;
            }
            ModuleItem::ModuleDecl(module_declaration) => match module_declaration {
                ModuleDecl::ExportDecl(export) => {
                    validate_strict_mode_early_errors_in_declaration(&export.decl, strict)?;
                }
                ModuleDecl::ExportDefaultDecl(ExportDefaultDecl { decl, .. }) => match decl {
                    DefaultDecl::Fn(function) => {
                        validate_strict_mode_early_errors_in_function(&function.function, strict)?;
                    }
                    DefaultDecl::Class(class) => {
                        validate_strict_mode_early_errors_in_class(&class.class, strict)?;
                    }
                    _ => {}
                },
                ModuleDecl::ExportDefaultExpr(export) => {
                    validate_strict_mode_early_errors_in_expression(&export.expr, strict)?;
                }
                _ => {}
            },
        }
    }

    Ok(())
}

pub(super) fn validate_strict_mode_early_errors_in_statements(
    statements: &[Stmt],
    strict: bool,
) -> Result<()> {
    for statement in statements {
        validate_strict_mode_early_errors_in_statement(statement, strict)?;
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_statement(statement: &Stmt, strict: bool) -> Result<()> {
    match statement {
        Stmt::Block(block) => {
            validate_strict_mode_early_errors_in_statements(&block.stmts, strict)?;
        }
        Stmt::Decl(declaration) => {
            validate_strict_mode_early_errors_in_declaration(declaration, strict)?;
        }
        Stmt::Expr(expression) => {
            validate_strict_mode_early_errors_in_expression(&expression.expr, strict)?;
        }
        Stmt::If(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.test, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.cons, strict)?;
            if let Some(alternate) = &statement.alt {
                validate_strict_mode_early_errors_in_statement(alternate, strict)?;
            }
        }
        Stmt::While(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.test, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::DoWhile(statement) => {
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
            validate_strict_mode_early_errors_in_expression(&statement.test, strict)?;
        }
        Stmt::For(statement) => {
            if let Some(init) = &statement.init {
                match init {
                    VarDeclOrExpr::VarDecl(variable_declaration) => {
                        validate_strict_mode_early_errors_in_variable_declaration(
                            variable_declaration,
                            strict,
                        )?;
                    }
                    VarDeclOrExpr::Expr(expression) => {
                        validate_strict_mode_early_errors_in_expression(expression, strict)?;
                    }
                }
            }
            if let Some(test) = &statement.test {
                validate_strict_mode_early_errors_in_expression(test, strict)?;
            }
            if let Some(update) = &statement.update {
                validate_strict_mode_early_errors_in_expression(update, strict)?;
            }
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::ForIn(statement) => {
            validate_strict_mode_early_errors_in_for_head(&statement.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&statement.right, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::ForOf(statement) => {
            validate_strict_mode_early_errors_in_for_head(&statement.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&statement.right, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::Switch(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.discriminant, strict)?;
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    validate_strict_mode_early_errors_in_expression(test, strict)?;
                }
                validate_strict_mode_early_errors_in_statements(&case.cons, strict)?;
            }
        }
        Stmt::Try(statement) => {
            validate_strict_mode_early_errors_in_statements(&statement.block.stmts, strict)?;
            if let Some(handler) = &statement.handler {
                if let Some(pattern) = &handler.param {
                    validate_strict_mode_early_errors_in_pattern(pattern, strict)?;
                }
                validate_strict_mode_early_errors_in_statements(&handler.body.stmts, strict)?;
            }
            if let Some(finalizer) = &statement.finalizer {
                validate_strict_mode_early_errors_in_statements(&finalizer.stmts, strict)?;
            }
        }
        Stmt::With(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.obj, strict)?;
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        Stmt::Return(statement) => {
            if let Some(argument) = &statement.arg {
                validate_strict_mode_early_errors_in_expression(argument, strict)?;
            }
        }
        Stmt::Throw(statement) => {
            validate_strict_mode_early_errors_in_expression(&statement.arg, strict)?;
        }
        Stmt::Labeled(statement) => {
            validate_strict_mode_early_errors_in_statement(&statement.body, strict)?;
        }
        _ => {}
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_declaration(
    declaration: &Decl,
    strict: bool,
) -> Result<()> {
    match declaration {
        Decl::Fn(function) => {
            validate_strict_mode_early_errors_in_function(&function.function, strict)?
        }
        Decl::Class(class) => validate_strict_mode_early_errors_in_class(&class.class, strict)?,
        Decl::Var(variable_declaration) => {
            validate_strict_mode_early_errors_in_variable_declaration(
                variable_declaration,
                strict,
            )?;
        }
        _ => {}
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_variable_declaration(
    declaration: &swc_ecma_ast::VarDecl,
    strict: bool,
) -> Result<()> {
    for declarator in &declaration.decls {
        validate_strict_mode_early_errors_in_pattern(&declarator.name, strict)?;
        if let Some(initializer) = &declarator.init {
            validate_strict_mode_early_errors_in_expression(initializer, strict)?;
        }
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_for_head(head: &ForHead, strict: bool) -> Result<()> {
    match head {
        ForHead::VarDecl(variable_declaration) => {
            validate_strict_mode_early_errors_in_variable_declaration(
                variable_declaration,
                strict,
            )?;
        }
        ForHead::Pat(pattern) => validate_strict_mode_early_errors_in_pattern(pattern, strict)?,
        ForHead::UsingDecl(_) => {}
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_pattern(pattern: &Pat, strict: bool) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            ensure!(
                !strict || !is_strict_mode_restricted_identifier(identifier.id.sym.as_ref()),
                "strict mode forbids binding `{}`",
                identifier.id.sym
            );
        }
        Pat::Assign(assign) => {
            validate_strict_mode_early_errors_in_pattern(&assign.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&assign.right, strict)?;
        }
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_strict_mode_early_errors_in_pattern(element, strict)?;
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                        validate_strict_mode_early_errors_in_pattern(&property.value, strict)?;
                    }
                    ObjectPatProp::Assign(property) => {
                        ensure!(
                            !strict
                                || !is_strict_mode_restricted_identifier(property.key.sym.as_ref()),
                            "strict mode forbids binding `{}`",
                            property.key.sym
                        );
                        if let Some(value) = &property.value {
                            validate_strict_mode_early_errors_in_expression(value, strict)?;
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        validate_strict_mode_early_errors_in_pattern(&rest.arg, strict)?;
                    }
                }
            }
        }
        Pat::Rest(rest) => validate_strict_mode_early_errors_in_pattern(&rest.arg, strict)?,
        _ => {}
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_function(function: &Function, strict: bool) -> Result<()> {
    let function_strict = strict || function_has_use_strict_directive(function);

    for parameter in &function.params {
        validate_strict_mode_early_errors_in_pattern(&parameter.pat, function_strict)?;
    }

    if let Some(body) = &function.body {
        validate_strict_mode_early_errors_in_statements(&body.stmts, function_strict)?;
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_class(class: &Class, strict: bool) -> Result<()> {
    if let Some(super_class) = &class.super_class {
        validate_strict_mode_early_errors_in_expression(super_class, strict)?;
    }

    for member in &class.body {
        match member {
            ClassMember::Constructor(constructor) => {
                validate_strict_mode_early_errors_in_constructor(constructor, true)?;
            }
            ClassMember::Method(method) => {
                validate_property_name_strict_mode_early_errors(&method.key, true)?;
                validate_strict_mode_early_errors_in_function(&method.function, true)?;
            }
            ClassMember::ClassProp(property) => {
                validate_property_name_strict_mode_early_errors(&property.key, true)?;
                if let Some(value) = &property.value {
                    validate_strict_mode_early_errors_in_expression(value, true)?;
                }
            }
            ClassMember::PrivateMethod(method) => {
                validate_strict_mode_early_errors_in_function(&method.function, true)?;
            }
            ClassMember::PrivateProp(property) => {
                if let Some(value) = &property.value {
                    validate_strict_mode_early_errors_in_expression(value, true)?;
                }
            }
            ClassMember::StaticBlock(block) => {
                validate_strict_mode_early_errors_in_statements(&block.body.stmts, true)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_constructor(
    constructor: &Constructor,
    strict: bool,
) -> Result<()> {
    for parameter in &constructor.params {
        match parameter {
            ParamOrTsParamProp::Param(parameter) => {
                validate_strict_mode_early_errors_in_pattern(&parameter.pat, strict)?;
            }
            ParamOrTsParamProp::TsParamProp(_) => {}
        }
    }

    if let Some(body) = &constructor.body {
        validate_strict_mode_early_errors_in_statements(&body.stmts, strict)?;
    }

    Ok(())
}

fn validate_property_name_strict_mode_early_errors(name: &PropName, strict: bool) -> Result<()> {
    if let PropName::Computed(computed) = name {
        validate_strict_mode_early_errors_in_expression(&computed.expr, strict)?;
    }

    Ok(())
}

fn validate_strict_mode_early_errors_in_expression(expression: &Expr, strict: bool) -> Result<()> {
    match expression {
        Expr::Call(call) => {
            if let Callee::Expr(callee) = &call.callee {
                validate_strict_mode_early_errors_in_expression(callee, strict)?;
            }
            for argument in &call.args {
                validate_strict_mode_early_errors_in_expression(&argument.expr, strict)?;
            }
        }
        Expr::New(new_expression) => {
            validate_strict_mode_early_errors_in_expression(&new_expression.callee, strict)?;
            for argument in new_expression.args.iter().flatten() {
                validate_strict_mode_early_errors_in_expression(&argument.expr, strict)?;
            }
        }
        Expr::Await(await_expression) => {
            validate_strict_mode_early_errors_in_expression(&await_expression.arg, strict)?;
        }
        Expr::Yield(yield_expression) => {
            if let Some(argument) = &yield_expression.arg {
                validate_strict_mode_early_errors_in_expression(argument, strict)?;
            }
        }
        Expr::Paren(parenthesized) => {
            validate_strict_mode_early_errors_in_expression(&parenthesized.expr, strict)?;
        }
        Expr::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_strict_mode_early_errors_in_expression(&element.expr, strict)?;
            }
        }
        Expr::Object(object) => {
            for property in &object.props {
                match property {
                    PropOrSpread::Spread(spread) => {
                        validate_strict_mode_early_errors_in_expression(&spread.expr, strict)?;
                    }
                    PropOrSpread::Prop(property) => match &**property {
                        Prop::Shorthand(identifier) => {
                            ensure!(
                                !strict
                                    || !is_strict_mode_restricted_identifier(
                                        identifier.sym.as_ref()
                                    ),
                                "strict mode forbids binding `{}`",
                                identifier.sym
                            );
                        }
                        Prop::KeyValue(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            validate_strict_mode_early_errors_in_expression(
                                &property.value,
                                strict,
                            )?;
                        }
                        Prop::Getter(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            if let Some(body) = &property.body {
                                validate_strict_mode_early_errors_in_statements(
                                    &body.stmts,
                                    strict,
                                )?;
                            }
                        }
                        Prop::Setter(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            validate_strict_mode_early_errors_in_pattern(&property.param, strict)?;
                            if let Some(body) = &property.body {
                                validate_strict_mode_early_errors_in_statements(
                                    &body.stmts,
                                    strict,
                                )?;
                            }
                        }
                        Prop::Method(property) => {
                            validate_property_name_strict_mode_early_errors(&property.key, strict)?;
                            validate_strict_mode_early_errors_in_function(
                                &property.function,
                                strict,
                            )?;
                        }
                        Prop::Assign(property) => {
                            ensure!(
                                !strict
                                    || !is_strict_mode_restricted_identifier(
                                        property.key.sym.as_ref()
                                    ),
                                "strict mode forbids binding `{}`",
                                property.key.sym
                            );
                            validate_strict_mode_early_errors_in_expression(
                                &property.value,
                                strict,
                            )?;
                        }
                    },
                }
            }
        }
        Expr::Member(member) => {
            validate_strict_mode_early_errors_in_expression(&member.obj, strict)?;
            if let MemberProp::Computed(property) = &member.prop {
                validate_strict_mode_early_errors_in_expression(&property.expr, strict)?;
            }
        }
        Expr::Unary(unary) => {
            if strict && unary.op == SwcUnaryOp::Delete && matches!(&*unary.arg, Expr::Ident(_)) {
                bail!("strict mode forbids deleting unqualified identifiers");
            }
            validate_strict_mode_early_errors_in_expression(&unary.arg, strict)?;
        }
        Expr::Update(update) => {
            if strict {
                if let Expr::Ident(identifier) = &*update.arg {
                    ensure!(
                        !is_strict_mode_restricted_identifier(identifier.sym.as_ref()),
                        "strict mode forbids updating `{}`",
                        identifier.sym
                    );
                }
            }
            validate_strict_mode_early_errors_in_expression(&update.arg, strict)?;
        }
        Expr::Bin(binary) => {
            validate_strict_mode_early_errors_in_expression(&binary.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&binary.right, strict)?;
        }
        Expr::Assign(assignment) => {
            validate_strict_mode_assignment_target(&assignment.left, strict)?;
            validate_strict_mode_early_errors_in_expression(&assignment.right, strict)?;
        }
        Expr::Cond(conditional) => {
            validate_strict_mode_early_errors_in_expression(&conditional.test, strict)?;
            validate_strict_mode_early_errors_in_expression(&conditional.cons, strict)?;
            validate_strict_mode_early_errors_in_expression(&conditional.alt, strict)?;
        }
        Expr::Seq(sequence) => {
            for expression in &sequence.exprs {
                validate_strict_mode_early_errors_in_expression(expression, strict)?;
            }
        }
        Expr::Fn(function) => {
            if let Some(identifier) = &function.ident {
                ensure!(
                    !strict || !is_strict_mode_restricted_identifier(identifier.sym.as_ref()),
                    "strict mode forbids binding `{}`",
                    identifier.sym
                );
            }
            validate_strict_mode_early_errors_in_function(&function.function, strict)?;
        }
        Expr::Arrow(arrow) => {
            for parameter in &arrow.params {
                validate_strict_mode_early_errors_in_pattern(parameter, strict)?;
            }
            match &*arrow.body {
                BlockStmtOrExpr::BlockStmt(block) => {
                    validate_strict_mode_early_errors_in_statements(&block.stmts, strict)?;
                }
                BlockStmtOrExpr::Expr(expression) => {
                    validate_strict_mode_early_errors_in_expression(expression, strict)?;
                }
            }
        }
        Expr::Class(class) => {
            if let Some(identifier) = &class.ident {
                ensure!(
                    !strict || !is_strict_mode_restricted_identifier(identifier.sym.as_ref()),
                    "strict mode forbids binding `{}`",
                    identifier.sym
                );
            }
            validate_strict_mode_early_errors_in_class(&class.class, strict)?;
        }
        Expr::Tpl(template) => {
            for expression in &template.exprs {
                validate_strict_mode_early_errors_in_expression(expression, strict)?;
            }
        }
        Expr::TaggedTpl(tagged) => {
            validate_strict_mode_early_errors_in_expression(&tagged.tag, strict)?;
            for expression in &tagged.tpl.exprs {
                validate_strict_mode_early_errors_in_expression(expression, strict)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn validate_strict_mode_assignment_target(target: &AssignTarget, strict: bool) -> Result<()> {
    if strict {
        if let AssignTarget::Simple(SimpleAssignTarget::Ident(identifier)) = target {
            ensure!(
                !is_strict_mode_restricted_identifier(identifier.id.sym.as_ref()),
                "strict mode forbids assigning to `{}`",
                identifier.id.sym
            );
        }
    }

    match target {
        AssignTarget::Simple(SimpleAssignTarget::Member(member)) => {
            validate_strict_mode_early_errors_in_expression(&member.obj, strict)?;
            if let MemberProp::Computed(property) = &member.prop {
                validate_strict_mode_early_errors_in_expression(&property.expr, strict)?;
            }
        }
        AssignTarget::Pat(_) | AssignTarget::Simple(_) => {}
    }

    Ok(())
}

fn is_strict_mode_restricted_identifier(name: &str) -> bool {
    matches!(name, "eval" | "arguments")
}

pub(super) fn script_has_use_strict_directive(statements: &[Stmt]) -> bool {
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

pub(super) fn function_has_use_strict_directive(function: &Function) -> bool {
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
