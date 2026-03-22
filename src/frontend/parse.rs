pub fn parse(source: &str) -> Result<Program> {
    let mut lowered_source = source.to_string();
    let parsed = parse_program_source(source).or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(source) else {
            return Err(parse_error);
        };
        lowered_source = rewritten;
        parse_program_source(&lowered_source).map_err(|rewrite_error| {
            anyhow::anyhow!(
                "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
            )
        })
    })?;

    Lowerer {
        source_text: Some(lowered_source),
        ..Default::default()
    }
    .lower_program(&parsed)
}

pub fn parse_script_goal(source: &str) -> Result<Program> {
    let mut lowered_source = source.to_string();
    let parsed = parse_script_program_source(source).or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(source) else {
            return Err(parse_error);
        };
        lowered_source = rewritten;
        parse_script_program_source(&lowered_source).map_err(|rewrite_error| {
            anyhow::anyhow!(
                "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
            )
        })
    })?;

    Lowerer {
        source_text: Some(lowered_source),
        ..Default::default()
    }
    .lower_program(&parsed)
}

pub fn parse_module_goal(source: &str) -> Result<Program> {
    parse_module_goal_with_path(Path::new("input.js"), source)
}

#[allow(dead_code)]
pub fn validate_script_goal(source: &str) -> Result<()> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Custom("eval.js".into()).into(),
        normalized.into_owned(),
    );
    parse_script(&file).map(|_| ())
}

pub fn parse_module_goal_with_path(path: &Path, source: &str) -> Result<Program> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Real(path.to_path_buf()).into(),
        normalized.into_owned(),
    );

    let parsed = parse_module(&file)?;
    Lowerer {
        source_text: Some(source.to_string()),
        ..Default::default()
    }
    .lower_program(&parsed)
}

pub fn bundle_module_entry(path: &Path) -> Result<Program> {
    ModuleLinker::default().bundle_entry(path)
}

pub fn bundle_script_entry(path: &Path) -> Result<Program> {
    ModuleLinker::default().bundle_script_entry(path)
}

fn parse_program_source(source: &str) -> Result<SwcProgram> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Custom("input.js".into()).into(),
        normalized.into_owned(),
    );

    parse_script(&file).or_else(|script_error| {
        parse_module(&file).map_err(|module_error| {
            anyhow::anyhow!(
                "failed to parse JavaScript source as script: {script_error:#}\nfailed to parse JavaScript source as module: {module_error:#}"
            )
        })
    })
}

fn parse_script_program_source(source: &str) -> Result<SwcProgram> {
    let normalized = normalize_leading_hashbang_comment(source);
    let source_map: Lrc<SourceMap> = Default::default();
    let file = source_map.new_source_file(
        FileName::Custom("input.js".into()).into(),
        normalized.into_owned(),
    );

    parse_script(&file)
}

fn normalize_leading_hashbang_comment(source: &str) -> Cow<'_, str> {
    if let Some(rest) = source.strip_prefix("#!") {
        return Cow::Owned(format!("//{rest}"));
    }

    if let Some(rest) = source.strip_prefix("\u{FEFF}#!") {
        return Cow::Owned(format!("\u{FEFF}//{rest}"));
    }

    Cow::Borrowed(source)
}

fn parse_script(file: &swc_common::SourceFile) -> Result<SwcProgram> {
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        Default::default(),
        StringInput::from(file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let script = parser
        .parse_script()
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        bail!("{error:?}");
    }
    validate_script_ast(&script, file)?;
    Ok(SwcProgram::Script(script))
}

fn parse_module(file: &swc_common::SourceFile) -> Result<SwcProgram> {
    let lexer = Lexer::new(
        Syntax::Es(Default::default()),
        Default::default(),
        StringInput::from(file),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let module = parser
        .parse_module()
        .map_err(|error| anyhow::anyhow!("{error:?}"))?;
    if let Some(error) = parser.take_errors().into_iter().next() {
        bail!("{error:?}");
    }
    validate_module_ast(&module, file)?;
    Ok(SwcProgram::Module(module))
}

fn validate_script_ast(script: &swc_ecma_ast::Script, file: &swc_common::SourceFile) -> Result<()> {
    for statement in &script.body {
        validate_statement_syntax(statement, file)?;
    }

    validate_strict_mode_early_errors_in_statements(
        &script.body,
        script_has_use_strict_directive(&script.body),
    )?;

    Ok(())
}

fn validate_module_ast(module: &Module, file: &swc_common::SourceFile) -> Result<()> {
    for item in &module.body {
        match item {
            ModuleItem::Stmt(statement) => validate_statement_syntax(statement, file)?,
            ModuleItem::ModuleDecl(module_declaration) => match module_declaration {
                ModuleDecl::Import(import) => {
                    validate_import_attributes(import.with.as_deref())?;
                }
                ModuleDecl::ExportNamed(export) => {
                    validate_import_attributes(export.with.as_deref())?;
                }
                ModuleDecl::ExportAll(export) => {
                    validate_import_attributes(export.with.as_deref())?;
                }
                ModuleDecl::ExportDecl(export) => validate_declaration_syntax(&export.decl, file)?,
                ModuleDecl::ExportDefaultDecl(ExportDefaultDecl { decl, .. }) => match decl {
                    DefaultDecl::Fn(function) => {
                        validate_function_syntax(&function.function, file)?
                    }
                    DefaultDecl::Class(class) => validate_class_syntax(&class.class, file)?,
                    _ => {}
                },
                ModuleDecl::ExportDefaultExpr(export) => {
                    validate_expression_syntax(&export.expr, file)?;
                }
                _ => {}
            },
        }
    }

    validate_strict_mode_early_errors_in_module_items(&module.body, true)?;

    Ok(())
}

fn validate_import_attributes(attributes: Option<&ObjectLit>) -> Result<()> {
    let Some(attributes) = attributes else {
        return Ok(());
    };
    let import_with = attributes
        .as_import_with()
        .context("unsupported import attributes syntax")?;
    let mut keys = HashSet::new();
    for item in import_with.values {
        let key = item.key.sym.to_string();
        ensure!(
            keys.insert(key.clone()),
            "duplicate import attribute key `{key}`"
        );
    }
    Ok(())
}

fn parse_module_file(path: &Path) -> Result<(Module, String)> {
    let source_map: Lrc<SourceMap> = Default::default();
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    let normalized = normalize_leading_hashbang_comment(&source);
    let file = source_map.new_source_file(
        FileName::Real(path.to_path_buf()).into(),
        normalized.into_owned(),
    );
    let SwcProgram::Module(module) = parse_module(&file)? else {
        unreachable!("parse_module must return a module");
    };
    Ok((module, source))
}

fn parse_script_file(path: &Path) -> Result<(swc_ecma_ast::Script, String)> {
    let source_map: Lrc<SourceMap> = Default::default();
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read `{}`", path.display()))?;
    let parse_once = |source: &str| -> Result<swc_ecma_ast::Script> {
        let normalized = normalize_leading_hashbang_comment(source);
        let file = source_map.new_source_file(
            FileName::Real(path.to_path_buf()).into(),
            normalized.into_owned(),
        );
        let SwcProgram::Script(script) = parse_script(&file)? else {
            unreachable!("parse_script must return a script");
        };
        Ok(script)
    };

    parse_once(&source)
        .map(|script| (script, source.clone()))
        .or_else(|parse_error| {
        let Some(rewritten) = rewrite_script_await_identifiers(&source) else {
            return Err(parse_error);
        };
        parse_once(&rewritten)
            .map(|script| (script, rewritten))
            .map_err(|rewrite_error| {
                anyhow::anyhow!(
                    "{parse_error:#}\nfailed again after rewriting script-goal `await` identifiers: {rewrite_error:#}"
                )
            })
    })
}
