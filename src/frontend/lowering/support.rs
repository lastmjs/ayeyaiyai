use super::*;

pub(crate) fn lower_parameters(
    lowerer: &mut Lowerer,
    function: &Function,
) -> Result<(Vec<Parameter>, Vec<Statement>)> {
    lower_parameter_patterns(
        lowerer,
        function.params.iter().map(|parameter| &parameter.pat),
    )
}

pub(crate) fn collect_parameter_binding_names<'a>(
    parameters: impl IntoIterator<Item = &'a Pat>,
) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for parameter in parameters {
        collect_pattern_binding_names(parameter, &mut names)?;
    }
    Ok(names)
}

pub(crate) fn collect_function_scope_binding_names(statements: &[Stmt]) -> Result<Vec<String>> {
    fn collect_statement(statement: &Stmt, names: &mut Vec<String>) -> Result<()> {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for declarator in &variable_declaration.decls {
                    collect_pattern_binding_names(&declarator.name, names)?;
                }
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                let name = function_declaration.ident.sym.to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            Stmt::Block(block) => {
                for statement in &block.stmts {
                    collect_statement(statement, names)?;
                }
            }
            Stmt::Labeled(labeled_statement) => {
                collect_statement(&labeled_statement.body, names)?;
            }
            Stmt::If(if_statement) => {
                collect_statement(&if_statement.cons, names)?;
                if let Some(alternate) = &if_statement.alt {
                    collect_statement(alternate, names)?;
                }
            }
            Stmt::While(while_statement) => {
                collect_statement(&while_statement.body, names)?;
            }
            Stmt::DoWhile(do_while_statement) => {
                collect_statement(&do_while_statement.body, names)?;
            }
            Stmt::For(for_statement) => {
                if let Some(VarDeclOrExpr::VarDecl(variable_declaration)) = &for_statement.init
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_statement.body, names)?;
            }
            Stmt::ForIn(for_in_statement) => {
                if let ForHead::VarDecl(variable_declaration) = &for_in_statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_in_statement.body, names)?;
            }
            Stmt::ForOf(for_of_statement) => {
                if let ForHead::VarDecl(variable_declaration) = &for_of_statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for declarator in &variable_declaration.decls {
                        collect_pattern_binding_names(&declarator.name, names)?;
                    }
                }
                collect_statement(&for_of_statement.body, names)?;
            }
            Stmt::Switch(switch_statement) => {
                for case in &switch_statement.cases {
                    for statement in &case.cons {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::Try(try_statement) => {
                for statement in &try_statement.block.stmts {
                    collect_statement(statement, names)?;
                }
                if let Some(handler) = &try_statement.handler {
                    for statement in &handler.body.stmts {
                        collect_statement(statement, names)?;
                    }
                }
                if let Some(finalizer) = &try_statement.finalizer {
                    for statement in &finalizer.stmts {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::With(with_statement) => {
                collect_statement(&with_statement.body, names)?;
            }
            _ => {}
        }

        Ok(())
    }

    let mut names = Vec::new();
    for statement in statements {
        collect_statement(statement, &mut names)?;
    }
    Ok(names)
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

pub(crate) fn collect_for_of_binding_names(pattern: &Pat, names: &mut Vec<String>) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            let name = identifier.id.sym.to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
        Pat::Assign(assign) => collect_for_of_binding_names(&assign.left, names)?,
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_for_of_binding_names(element, names)?;
            }
        }
        _ => bail!("unsupported for-of binding pattern"),
    }

    Ok(())
}

pub(crate) fn collect_switch_bindings(switch_statement: &SwitchStmt) -> Result<Vec<String>> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();

    for case in &switch_statement.cases {
        for statement in &case.cons {
            let Stmt::Decl(Decl::Var(variable_declaration)) = statement else {
                continue;
            };
            if matches!(variable_declaration.kind, VarDeclKind::Var) {
                continue;
            }

            for declarator in &variable_declaration.decls {
                let mut names = Vec::new();
                collect_pattern_binding_names(&declarator.name, &mut names)?;
                for name in names {
                    if seen.insert(name.clone()) {
                        bindings.push(name);
                    }
                }
            }
        }
    }

    Ok(bindings)
}

pub(crate) fn collect_direct_statement_lexical_bindings(
    statements: &[Stmt],
) -> Result<Vec<String>> {
    let mut bindings = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for declarator in &variable_declaration.decls {
                    collect_pattern_binding_names(&declarator.name, &mut bindings)?;
                }
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                let name = function_declaration.ident.sym.to_string();
                if !bindings.contains(&name) {
                    bindings.push(name);
                }
            }
            Stmt::Decl(Decl::Class(class_declaration)) => {
                let name = class_declaration.ident.sym.to_string();
                if !bindings.contains(&name) {
                    bindings.push(name);
                }
            }
            _ => {}
        }
    }

    Ok(bindings)
}

pub(crate) fn collect_for_per_iteration_bindings(init: &VarDeclOrExpr) -> Result<Vec<String>> {
    let VarDeclOrExpr::VarDecl(variable_declaration) = init else {
        return Ok(Vec::new());
    };

    if matches!(variable_declaration.kind, VarDeclKind::Var) {
        return Ok(Vec::new());
    }

    Ok(variable_declaration
        .decls
        .iter()
        .map(|declarator| {
            let mut names = Vec::new();
            collect_pattern_binding_names(&declarator.name, &mut names)?;
            Ok(names)
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

pub(crate) fn pattern_name_hint(pattern: &Pat) -> Option<&str> {
    match pattern {
        Pat::Ident(identifier) => Some(identifier.id.sym.as_ref()),
        _ => None,
    }
}

pub(crate) fn await_resume_expression() -> Expression {
    Expression::Call {
        callee: Box::new(Expression::Identifier("__ayyAwaitResume".to_string())),
        arguments: vec![CallArgument::Expression(Expression::Sent)],
    }
}

pub(crate) fn define_property_statement(
    target: Expression,
    property: Expression,
    descriptor: Expression,
) -> Statement {
    Statement::Expression(Expression::Call {
        callee: Box::new(Expression::Member {
            object: Box::new(Expression::Identifier("Object".to_string())),
            property: Box::new(Expression::String("defineProperty".to_string())),
        }),
        arguments: vec![
            CallArgument::Expression(target),
            CallArgument::Expression(property),
            CallArgument::Expression(descriptor),
        ],
    })
}

pub(crate) fn data_property_descriptor(
    value: Expression,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("value".to_string()),
            value,
        },
        ObjectEntry::Data {
            key: Expression::String("writable".to_string()),
            value: Expression::Bool(writable),
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

pub(crate) fn getter_property_descriptor(
    getter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("get".to_string()),
            value: getter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

pub(crate) fn setter_property_descriptor(
    setter: Expression,
    enumerable: bool,
    configurable: bool,
) -> Expression {
    Expression::Object(vec![
        ObjectEntry::Data {
            key: Expression::String("set".to_string()),
            value: setter,
        },
        ObjectEntry::Data {
            key: Expression::String("enumerable".to_string()),
            value: Expression::Bool(enumerable),
        },
        ObjectEntry::Data {
            key: Expression::String("configurable".to_string()),
            value: Expression::Bool(configurable),
        },
    ])
}

pub(crate) fn asyncify_statement(statement: Statement) -> (Vec<Statement>, bool) {
    match statement {
        Statement::Expression(Expression::Await(value)) => (
            vec![
                Statement::Yield { value: *value },
                Statement::Expression(await_resume_expression()),
            ],
            true,
        ),
        Statement::Var {
            name,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Var {
                    name,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Let {
            name,
            mutable,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Let {
                    name,
                    mutable,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Assign {
            name,
            value: Expression::Await(value),
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::Assign {
                    name,
                    value: await_resume_expression(),
                },
            ],
            true,
        ),
        Statement::Return(Expression::Await(value)) => (
            vec![
                Statement::Yield { value: *value },
                Statement::Return(await_resume_expression()),
            ],
            true,
        ),
        Statement::If {
            condition: Expression::Await(value),
            then_branch,
            else_branch,
        } => (
            vec![
                Statement::Yield { value: *value },
                Statement::If {
                    condition: await_resume_expression(),
                    then_branch,
                    else_branch,
                },
            ],
            true,
        ),
        other => (vec![other], false),
    }
}

pub(crate) fn asyncify_statements(statements: Vec<Statement>) -> (Vec<Statement>, bool) {
    let mut asyncified = Vec::new();
    let mut changed = false;

    for statement in statements {
        let (mut lowered, statement_changed) = asyncify_statement(statement);
        changed |= statement_changed;
        asyncified.append(&mut lowered);
    }

    (asyncified, changed)
}

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

pub(crate) fn lower_binary_operator(operator: SwcBinaryOp) -> Result<BinaryOp> {
    Ok(match operator {
        SwcBinaryOp::Add => BinaryOp::Add,
        SwcBinaryOp::Sub => BinaryOp::Subtract,
        SwcBinaryOp::Mul => BinaryOp::Multiply,
        SwcBinaryOp::Div => BinaryOp::Divide,
        SwcBinaryOp::Mod => BinaryOp::Modulo,
        SwcBinaryOp::Exp => BinaryOp::Exponentiate,
        SwcBinaryOp::BitAnd => BinaryOp::BitwiseAnd,
        SwcBinaryOp::BitOr => BinaryOp::BitwiseOr,
        SwcBinaryOp::BitXor => BinaryOp::BitwiseXor,
        SwcBinaryOp::LShift => BinaryOp::LeftShift,
        SwcBinaryOp::RShift => BinaryOp::RightShift,
        SwcBinaryOp::ZeroFillRShift => BinaryOp::UnsignedRightShift,
        SwcBinaryOp::In => BinaryOp::In,
        SwcBinaryOp::InstanceOf => BinaryOp::InstanceOf,
        SwcBinaryOp::EqEq => BinaryOp::LooseEqual,
        SwcBinaryOp::NotEq => BinaryOp::LooseNotEqual,
        SwcBinaryOp::EqEqEq => BinaryOp::Equal,
        SwcBinaryOp::NotEqEq => BinaryOp::NotEqual,
        SwcBinaryOp::Lt => BinaryOp::LessThan,
        SwcBinaryOp::LtEq => BinaryOp::LessThanOrEqual,
        SwcBinaryOp::Gt => BinaryOp::GreaterThan,
        SwcBinaryOp::GtEq => BinaryOp::GreaterThanOrEqual,
        SwcBinaryOp::LogicalAnd => BinaryOp::LogicalAnd,
        SwcBinaryOp::LogicalOr => BinaryOp::LogicalOr,
        SwcBinaryOp::NullishCoalescing => BinaryOp::NullishCoalescing,
    })
}

pub(crate) fn lower_unary_operator(operator: SwcUnaryOp) -> Result<UnaryOp> {
    Ok(match operator {
        SwcUnaryOp::Minus => UnaryOp::Negate,
        SwcUnaryOp::Plus => UnaryOp::Plus,
        SwcUnaryOp::Bang => UnaryOp::Not,
        SwcUnaryOp::Tilde => UnaryOp::BitwiseNot,
        SwcUnaryOp::TypeOf => UnaryOp::TypeOf,
        SwcUnaryOp::Void => UnaryOp::Void,
        SwcUnaryOp::Delete => UnaryOp::Delete,
    })
}

pub(crate) fn lower_update_operator(operator: SwcUpdateOp) -> UpdateOp {
    match operator {
        SwcUpdateOp::PlusPlus => UpdateOp::Increment,
        SwcUpdateOp::MinusMinus => UpdateOp::Decrement,
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

pub(crate) fn lower_function_kind(is_generator: bool, is_async: bool) -> FunctionKind {
    if is_generator {
        FunctionKind::Generator
    } else if is_async {
        FunctionKind::Async
    } else {
        FunctionKind::Ordinary
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

pub(crate) fn binding_ident(pattern: &Pat) -> Result<&BindingIdent> {
    match pattern {
        Pat::Ident(identifier) => Ok(identifier),
        _ => bail!("only identifier bindings are supported"),
    }
}
