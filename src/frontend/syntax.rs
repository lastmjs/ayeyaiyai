use super::*;

pub(super) fn collect_var_decl_bound_names(
    variable_declaration: &swc_ecma_ast::VarDecl,
) -> Result<Vec<String>> {
    let mut names = Vec::new();

    for declarator in &variable_declaration.decls {
        collect_pattern_binding_names(&declarator.name, &mut names)?;
    }

    Ok(names)
}

pub(super) fn collect_module_declared_names(module: &Module) -> Result<HashSet<String>> {
    let mut names = HashSet::new();

    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(function_declaration))) => {
                names.insert(function_declaration.ident.sym.to_string());
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Class(class_declaration))) => {
                names.insert(class_declaration.ident.sym.to_string());
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(variable_declaration))) => {
                names.extend(collect_var_decl_bound_names(variable_declaration)?);
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
                Decl::Fn(function_declaration) => {
                    names.insert(function_declaration.ident.sym.to_string());
                }
                Decl::Class(class_declaration) => {
                    names.insert(class_declaration.ident.sym.to_string());
                }
                Decl::Var(variable_declaration) => {
                    names.extend(collect_var_decl_bound_names(variable_declaration)?);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(names)
}

pub(super) fn ensure_module_lexical_names_are_unique(module: &Module) -> Result<()> {
    let mut seen = HashSet::new();

    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(function_declaration))) => {
                ensure!(
                    seen.insert(function_declaration.ident.sym.to_string()),
                    "duplicate lexical name `{}`",
                    function_declaration.ident.sym
                );
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Class(class_declaration))) => {
                ensure!(
                    seen.insert(class_declaration.ident.sym.to_string()),
                    "duplicate lexical name `{}`",
                    class_declaration.ident.sym
                );
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(variable_declaration)))
                if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                insert_unique_pattern_names(variable_declaration, &mut seen)?;
            }
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                for specifier in &import.specifiers {
                    let local_name = match specifier {
                        ImportSpecifier::Named(named) => named.local.sym.to_string(),
                        ImportSpecifier::Default(default) => default.local.sym.to_string(),
                        ImportSpecifier::Namespace(namespace) => namespace.local.sym.to_string(),
                    };
                    ensure!(
                        seen.insert(local_name.clone()),
                        "duplicate lexical name `{local_name}`"
                    );
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => match &export.decl {
                Decl::Fn(function_declaration) => {
                    ensure!(
                        seen.insert(function_declaration.ident.sym.to_string()),
                        "duplicate lexical name `{}`",
                        function_declaration.ident.sym
                    );
                }
                Decl::Class(class_declaration) => {
                    ensure!(
                        seen.insert(class_declaration.ident.sym.to_string()),
                        "duplicate lexical name `{}`",
                        class_declaration.ident.sym
                    );
                }
                Decl::Var(variable_declaration)
                    if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
                {
                    insert_unique_pattern_names(variable_declaration, &mut seen)?;
                }
                _ => {}
            },
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(export_default)) => {
                match &export_default.decl {
                    DefaultDecl::Fn(function_expression) => {
                        if let Some(identifier) = &function_expression.ident {
                            ensure!(
                                seen.insert(identifier.sym.to_string()),
                                "duplicate lexical name `{}`",
                                identifier.sym
                            );
                        }
                    }
                    DefaultDecl::Class(class_expression) => {
                        if let Some(identifier) = &class_expression.ident {
                            ensure!(
                                seen.insert(identifier.sym.to_string()),
                                "duplicate lexical name `{}`",
                                identifier.sym
                            );
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(())
}

pub(super) fn validate_statement_syntax(
    statement: &Stmt,
    file: &swc_common::SourceFile,
) -> Result<()> {
    match statement {
        Stmt::Block(block) => {
            validate_block_statement_early_errors(&block.stmts)?;
            for statement in &block.stmts {
                validate_statement_syntax(statement, file)?;
            }
        }
        Stmt::Decl(declaration) => validate_declaration_syntax(declaration, file)?,
        Stmt::Expr(expression) => validate_expression_syntax(&expression.expr, file)?,
        Stmt::If(statement) => {
            validate_expression_syntax(&statement.test, file)?;
            validate_statement_syntax(&statement.cons, file)?;
            if let Some(alternate) = &statement.alt {
                validate_statement_syntax(alternate, file)?;
            }
        }
        Stmt::While(statement) => {
            validate_expression_syntax(&statement.test, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::DoWhile(statement) => {
            validate_statement_syntax(&statement.body, file)?;
            validate_expression_syntax(&statement.test, file)?;
        }
        Stmt::For(statement) => {
            validate_classic_for_header(statement, file)?;
            if let Some(init) = &statement.init {
                match init {
                    VarDeclOrExpr::VarDecl(variable_declaration) => {
                        validate_variable_declaration_syntax(variable_declaration, file)?;
                    }
                    VarDeclOrExpr::Expr(expression) => {
                        validate_expression_syntax(expression, file)?
                    }
                }
            }
            if let Some(test) = &statement.test {
                validate_expression_syntax(test, file)?;
            }
            if let Some(update) = &statement.update {
                validate_expression_syntax(update, file)?;
            }
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::ForIn(statement) => {
            validate_for_head_syntax(&statement.left, file)?;
            validate_expression_syntax(&statement.right, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::ForOf(statement) => {
            validate_for_head_syntax(&statement.left, file)?;
            validate_expression_syntax(&statement.right, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::Switch(statement) => {
            validate_expression_syntax(&statement.discriminant, file)?;
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    validate_expression_syntax(test, file)?;
                }
                for statement in &case.cons {
                    validate_statement_syntax(statement, file)?;
                }
            }
        }
        Stmt::Try(statement) => {
            for statement in &statement.block.stmts {
                validate_statement_syntax(statement, file)?;
            }
            if let Some(handler) = &statement.handler {
                if let Some(pattern) = &handler.param {
                    validate_pattern_syntax(pattern, file)?;
                }
                for statement in &handler.body.stmts {
                    validate_statement_syntax(statement, file)?;
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.stmts {
                    validate_statement_syntax(statement, file)?;
                }
            }
        }
        Stmt::With(statement) => {
            validate_expression_syntax(&statement.obj, file)?;
            validate_statement_syntax(&statement.body, file)?;
        }
        Stmt::Return(statement) => {
            if let Some(argument) = &statement.arg {
                validate_expression_syntax(argument, file)?;
            }
        }
        Stmt::Throw(statement) => validate_expression_syntax(&statement.arg, file)?,
        Stmt::Labeled(statement) => validate_statement_syntax(&statement.body, file)?,
        _ => {}
    }

    Ok(())
}

pub(super) fn collect_pattern_binding_names(pattern: &Pat, names: &mut Vec<String>) -> Result<()> {
    match pattern {
        Pat::Ident(identifier) => {
            let name = identifier.id.sym.to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
        Pat::Assign(assign) => collect_pattern_binding_names(&assign.left, names)?,
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                collect_pattern_binding_names(element, names)?;
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        collect_pattern_binding_names(&property.value, names)?;
                    }
                    ObjectPatProp::Assign(property) => {
                        let name = property.key.id.sym.to_string();
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                    ObjectPatProp::Rest(rest) => {
                        collect_pattern_binding_names(&rest.arg, names)?;
                    }
                }
            }
        }
        Pat::Rest(rest) => collect_pattern_binding_names(&rest.arg, names)?,
        Pat::Expr(_) | Pat::Invalid(_) => bail!("unsupported binding pattern"),
    }

    Ok(())
}

fn validate_block_statement_early_errors(statements: &[Stmt]) -> Result<()> {
    let lexical_names = ensure_direct_statement_lexical_names_are_unique(statements)?;
    let var_names = collect_var_declared_names_from_statement_list(statements)?;
    let var_names = var_names.into_iter().collect::<HashSet<_>>();

    for name in lexical_names {
        ensure!(
            !var_names.contains(&name),
            "duplicate lexical name `{name}`"
        );
    }

    Ok(())
}

fn ensure_direct_statement_lexical_names_are_unique(statements: &[Stmt]) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();

    for statement in statements {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if !matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for name in collect_var_decl_bound_names(variable_declaration)? {
                    ensure!(seen.insert(name.clone()), "duplicate lexical name `{name}`");
                    names.push(name);
                }
            }
            Stmt::Decl(Decl::Fn(function_declaration)) => {
                let name = function_declaration.ident.sym.to_string();
                ensure!(seen.insert(name.clone()), "duplicate lexical name `{name}`");
                names.push(name);
            }
            Stmt::Decl(Decl::Class(class_declaration)) => {
                let name = class_declaration.ident.sym.to_string();
                ensure!(seen.insert(name.clone()), "duplicate lexical name `{name}`");
                names.push(name);
            }
            _ => {}
        }
    }

    Ok(names)
}

fn collect_var_declared_names_from_statement_list(statements: &[Stmt]) -> Result<Vec<String>> {
    fn collect_statement(statement: &Stmt, names: &mut Vec<String>) -> Result<()> {
        match statement {
            Stmt::Decl(Decl::Var(variable_declaration))
                if matches!(variable_declaration.kind, VarDeclKind::Var) =>
            {
                for name in collect_var_decl_bound_names(variable_declaration)? {
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
            Stmt::Block(block) => {
                for statement in &block.stmts {
                    collect_statement(statement, names)?;
                }
            }
            Stmt::Labeled(statement) => collect_statement(&statement.body, names)?,
            Stmt::If(statement) => {
                collect_statement(&statement.cons, names)?;
                if let Some(alternate) = &statement.alt {
                    collect_statement(alternate, names)?;
                }
            }
            Stmt::While(statement) => collect_statement(&statement.body, names)?,
            Stmt::DoWhile(statement) => collect_statement(&statement.body, names)?,
            Stmt::For(statement) => {
                if let Some(VarDeclOrExpr::VarDecl(variable_declaration)) = &statement.init
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for name in collect_var_decl_bound_names(variable_declaration)? {
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                collect_statement(&statement.body, names)?;
            }
            Stmt::ForIn(statement) => {
                if let ForHead::VarDecl(variable_declaration) = &statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for name in collect_var_decl_bound_names(variable_declaration)? {
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                collect_statement(&statement.body, names)?;
            }
            Stmt::ForOf(statement) => {
                if let ForHead::VarDecl(variable_declaration) = &statement.left
                    && matches!(variable_declaration.kind, VarDeclKind::Var)
                {
                    for name in collect_var_decl_bound_names(variable_declaration)? {
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
                collect_statement(&statement.body, names)?;
            }
            Stmt::Switch(statement) => {
                for case in &statement.cases {
                    for statement in &case.cons {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::Try(statement) => {
                for statement in &statement.block.stmts {
                    collect_statement(statement, names)?;
                }
                if let Some(handler) = &statement.handler {
                    for statement in &handler.body.stmts {
                        collect_statement(statement, names)?;
                    }
                }
                if let Some(finalizer) = &statement.finalizer {
                    for statement in &finalizer.stmts {
                        collect_statement(statement, names)?;
                    }
                }
            }
            Stmt::With(statement) => collect_statement(&statement.body, names)?,
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

pub(super) fn validate_declaration_syntax(
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

fn validate_variable_declaration_syntax(
    declaration: &swc_ecma_ast::VarDecl,
    file: &swc_common::SourceFile,
) -> Result<()> {
    for declarator in &declaration.decls {
        validate_pattern_syntax(&declarator.name, file)?;
        if let Some(initializer) = &declarator.init {
            validate_expression_syntax(initializer, file)?;
        }
    }

    Ok(())
}

fn validate_for_head_syntax(head: &ForHead, file: &swc_common::SourceFile) -> Result<()> {
    match head {
        ForHead::VarDecl(variable_declaration) => {
            validate_variable_declaration_syntax(variable_declaration, file)?;
        }
        ForHead::Pat(pattern) => validate_pattern_syntax(pattern, file)?,
        ForHead::UsingDecl(_) => {}
    }

    Ok(())
}

fn validate_pattern_syntax(pattern: &Pat, file: &swc_common::SourceFile) -> Result<()> {
    match pattern {
        Pat::Assign(assign) => {
            validate_pattern_syntax(&assign.left, file)?;
            validate_expression_syntax(&assign.right, file)?;
        }
        Pat::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_pattern_syntax(element, file)?;
            }
        }
        Pat::Object(object) => {
            for property in &object.props {
                match property {
                    ObjectPatProp::KeyValue(property) => {
                        validate_pattern_syntax(&property.value, file)?;
                    }
                    ObjectPatProp::Assign(property) => {
                        if let Some(value) = &property.value {
                            validate_expression_syntax(value, file)?;
                        }
                    }
                    ObjectPatProp::Rest(rest) => validate_pattern_syntax(&rest.arg, file)?,
                }
            }
        }
        Pat::Rest(rest) => validate_pattern_syntax(&rest.arg, file)?,
        _ => {}
    }

    Ok(())
}

pub(super) fn validate_function_syntax(
    function: &Function,
    file: &swc_common::SourceFile,
) -> Result<()> {
    ensure_parameter_names_are_valid(
        function.params.iter().map(|parameter| &parameter.pat),
        function
            .params
            .iter()
            .all(|parameter| matches!(parameter.pat, Pat::Ident(_))),
        function_has_use_strict_directive(function),
    )?;
    for parameter in &function.params {
        validate_pattern_syntax(&parameter.pat, file)?;
    }
    if let Some(body) = &function.body {
        for statement in &body.stmts {
            validate_statement_syntax(statement, file)?;
        }
    }

    Ok(())
}

fn ensure_parameter_names_are_valid<'a>(
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

pub(super) fn validate_class_syntax(class: &Class, file: &swc_common::SourceFile) -> Result<()> {
    if let Some(super_class) = &class.super_class {
        validate_expression_syntax(super_class, file)?;
    }

    for member in &class.body {
        match member {
            ClassMember::Constructor(constructor) => {
                validate_constructor_syntax(constructor, file)?;
            }
            ClassMember::Method(method) => {
                validate_property_name_syntax(&method.key, file)?;
                validate_function_syntax(&method.function, file)?;
            }
            ClassMember::ClassProp(property) => {
                validate_property_name_syntax(&property.key, file)?;
                if let Some(value) = &property.value {
                    validate_expression_syntax(value, file)?;
                }
            }
            ClassMember::PrivateMethod(method) => {
                validate_function_syntax(&method.function, file)?;
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
) -> Result<()> {
    for parameter in &constructor.params {
        match parameter {
            ParamOrTsParamProp::Param(parameter) => validate_pattern_syntax(&parameter.pat, file)?,
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

fn validate_property_name_syntax(name: &PropName, file: &swc_common::SourceFile) -> Result<()> {
    if let PropName::Computed(computed) = name {
        validate_expression_syntax(&computed.expr, file)?;
    }

    Ok(())
}

pub(super) fn validate_expression_syntax(
    expression: &Expr,
    file: &swc_common::SourceFile,
) -> Result<()> {
    match expression {
        Expr::Call(call) => {
            if let Callee::Expr(callee) = &call.callee {
                validate_expression_syntax(callee, file)?;
            }
            for argument in &call.args {
                validate_expression_syntax(&argument.expr, file)?;
            }
        }
        Expr::New(new_expression) => {
            validate_expression_syntax(&new_expression.callee, file)?;
            for argument in new_expression.args.iter().flatten() {
                validate_expression_syntax(&argument.expr, file)?;
            }
        }
        Expr::Await(await_expression) => {
            validate_expression_syntax(&await_expression.arg, file)?;
        }
        Expr::Yield(yield_expression) => {
            if let Some(argument) = &yield_expression.arg {
                validate_expression_syntax(argument, file)?;
            }
        }
        Expr::Paren(parenthesized) => validate_expression_syntax(&parenthesized.expr, file)?,
        Expr::Array(array) => {
            for element in array.elems.iter().flatten() {
                validate_expression_syntax(&element.expr, file)?;
            }
        }
        Expr::Object(object) => {
            for property in &object.props {
                match property {
                    PropOrSpread::Spread(spread) => validate_expression_syntax(&spread.expr, file)?,
                    PropOrSpread::Prop(property) => match &**property {
                        Prop::Shorthand(_) => {}
                        Prop::KeyValue(property) => {
                            validate_property_name_syntax(&property.key, file)?;
                            validate_expression_syntax(&property.value, file)?;
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
                        Prop::Assign(property) => {
                            validate_expression_syntax(&property.value, file)?
                        }
                    },
                }
            }
        }
        Expr::Member(member) => {
            validate_expression_syntax(&member.obj, file)?;
            if let MemberProp::Computed(property) = &member.prop {
                validate_expression_syntax(&property.expr, file)?;
            }
        }
        Expr::Unary(unary) => validate_expression_syntax(&unary.arg, file)?,
        Expr::Update(update) => validate_expression_syntax(&update.arg, file)?,
        Expr::Bin(binary) => {
            validate_expression_syntax(&binary.left, file)?;
            validate_expression_syntax(&binary.right, file)?;
        }
        Expr::Assign(assignment) => {
            match &assignment.left {
                AssignTarget::Simple(SimpleAssignTarget::Member(member)) => {
                    validate_expression_syntax(&member.obj, file)?;
                    if let MemberProp::Computed(property) = &member.prop {
                        validate_expression_syntax(&property.expr, file)?;
                    }
                }
                AssignTarget::Simple(_) | AssignTarget::Pat(_) => {}
            }
            validate_expression_syntax(&assignment.right, file)?;
        }
        Expr::Cond(conditional) => {
            validate_expression_syntax(&conditional.test, file)?;
            validate_expression_syntax(&conditional.cons, file)?;
            validate_expression_syntax(&conditional.alt, file)?;
        }
        Expr::Seq(sequence) => {
            for expression in &sequence.exprs {
                validate_expression_syntax(expression, file)?;
            }
        }
        Expr::Fn(function) => validate_function_syntax(&function.function, file)?,
        Expr::Arrow(arrow) => {
            ensure_parameter_names_are_valid(
                arrow.params.iter(),
                arrow
                    .params
                    .iter()
                    .all(|parameter| matches!(parameter, Pat::Ident(_))),
                false,
            )?;
            for parameter in &arrow.params {
                validate_pattern_syntax(parameter, file)?;
            }
            match &*arrow.body {
                BlockStmtOrExpr::BlockStmt(block) => {
                    for statement in &block.stmts {
                        validate_statement_syntax(statement, file)?;
                    }
                }
                BlockStmtOrExpr::Expr(expression) => validate_expression_syntax(expression, file)?,
            }
        }
        Expr::Class(class) => validate_class_syntax(&class.class, file)?,
        Expr::Tpl(template) => {
            for expression in &template.exprs {
                validate_expression_syntax(expression, file)?;
            }
        }
        Expr::TaggedTpl(tagged) => {
            validate_expression_syntax(&tagged.tag, file)?;
            for expression in &tagged.tpl.exprs {
                validate_expression_syntax(expression, file)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn validate_classic_for_header(
    statement: &swc_ecma_ast::ForStmt,
    file: &swc_common::SourceFile,
) -> Result<()> {
    let source: &str = file.src.as_ref();
    let start = statement.span.lo.to_usize() - file.start_pos.to_usize();
    let end = statement.span.hi.to_usize() - file.start_pos.to_usize();
    let statement_source = source
        .get(start..end)
        .context("classic `for` statement span fell outside the source file")?;
    let semicolon_count = count_classic_for_header_semicolons(statement_source)?;

    ensure!(
        semicolon_count == 2,
        "invalid classic `for` header: expected 2 top-level semicolons, found {semicolon_count}"
    );

    Ok(())
}

fn count_classic_for_header_semicolons(statement_source: &str) -> Result<usize> {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        SingleQuoted,
        DoubleQuoted,
        Template,
        LineComment,
        BlockComment,
    }

    let bytes = statement_source.as_bytes();
    let mut state = State::Code;
    let mut index = 0;

    while index < bytes.len() {
        let character = bytes[index];
        let next = bytes.get(index + 1).copied();

        match state {
            State::Code => match character {
                b'\'' => state = State::SingleQuoted,
                b'"' => state = State::DoubleQuoted,
                b'`' => state = State::Template,
                b'/' if next == Some(b'/') => {
                    state = State::LineComment;
                    index += 1;
                }
                b'/' if next == Some(b'*') => {
                    state = State::BlockComment;
                    index += 1;
                }
                b'(' => {
                    index += 1;
                    break;
                }
                _ => {}
            },
            State::SingleQuoted => {
                if character == b'\\' {
                    index += 1;
                } else if character == b'\'' {
                    state = State::Code;
                }
            }
            State::DoubleQuoted => {
                if character == b'\\' {
                    index += 1;
                } else if character == b'"' {
                    state = State::Code;
                }
            }
            State::Template => {
                if character == b'\\' {
                    index += 1;
                } else if character == b'`' {
                    state = State::Code;
                }
            }
            State::LineComment => {
                if character == b'\n' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                if character == b'*' && next == Some(b'/') {
                    state = State::Code;
                    index += 1;
                }
            }
        }

        index += 1;
    }

    ensure!(
        index <= bytes.len(),
        "classic `for` header did not contain an opening parenthesis"
    );

    state = State::Code;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut semicolon_count = 0usize;

    while index < bytes.len() {
        let character = bytes[index];
        let next = bytes.get(index + 1).copied();

        match state {
            State::Code => match character {
                b'\'' => state = State::SingleQuoted,
                b'"' => state = State::DoubleQuoted,
                b'`' => state = State::Template,
                b'/' if next == Some(b'/') => {
                    state = State::LineComment;
                    index += 1;
                }
                b'/' if next == Some(b'*') => {
                    state = State::BlockComment;
                    index += 1;
                }
                b'(' => paren_depth += 1,
                b'[' => bracket_depth += 1,
                b'{' => brace_depth += 1,
                b')' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                    return Ok(semicolon_count);
                }
                b')' => {
                    paren_depth = paren_depth.saturating_sub(1);
                }
                b']' => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                }
                b'}' => {
                    brace_depth = brace_depth.saturating_sub(1);
                }
                b';' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                    semicolon_count += 1;
                }
                _ => {}
            },
            State::SingleQuoted => {
                if character == b'\\' {
                    index += 1;
                } else if character == b'\'' {
                    state = State::Code;
                }
            }
            State::DoubleQuoted => {
                if character == b'\\' {
                    index += 1;
                } else if character == b'"' {
                    state = State::Code;
                }
            }
            State::Template => {
                if character == b'\\' {
                    index += 1;
                } else if character == b'`' {
                    state = State::Code;
                }
            }
            State::LineComment => {
                if character == b'\n' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                if character == b'*' && next == Some(b'/') {
                    state = State::Code;
                    index += 1;
                }
            }
        }

        index += 1;
    }

    bail!("classic `for` header did not contain a closing parenthesis")
}

fn insert_unique_pattern_names(
    variable_declaration: &swc_ecma_ast::VarDecl,
    seen: &mut HashSet<String>,
) -> Result<()> {
    for name in collect_var_decl_bound_names(variable_declaration)? {
        ensure!(seen.insert(name.clone()), "duplicate lexical name `{name}`");
    }
    Ok(())
}
