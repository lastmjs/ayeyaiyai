use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};

use crate::ir::hir::{
    ArrayElement, CallArgument, Expression, FunctionDeclaration, ObjectEntry, Program, Statement,
    SwitchCase,
};

include!("lowering.rs");
include!("validation.rs");

pub fn validate_refined_aot(program: &Program) -> Result<()> {
    RefinedAotValidator::new(program).validate()
}

pub fn lower_static_function_constructors(program: Program) -> Result<Program> {
    StaticFunctionConstructorLowerer::new(&program).lower(program)
}

fn collect_statement_bindings<'a>(statements: impl Iterator<Item = &'a Statement>) -> Vec<String> {
    let mut bindings = Vec::new();
    let mut seen = HashSet::new();
    for statement in statements {
        match statement {
            Statement::Var { name, .. } | Statement::Let { name, .. } => {
                if seen.insert(name.clone()) {
                    bindings.push(name.clone());
                }
            }
            _ => {}
        }
    }
    bindings
}

fn function_constructor_literal_source_parts(
    arguments: &[CallArgument],
) -> Option<(String, String)> {
    let parts = arguments
        .iter()
        .map(|argument| match argument {
            CallArgument::Expression(Expression::String(text)) => Some(text.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;

    let Some((body_source, parameter_sources)) = parts.split_last() else {
        return Some((String::new(), String::new()));
    };

    Some((parameter_sources.join(","), body_source.clone()))
}

#[cfg(test)]
mod tests {
    use super::validate_refined_aot;
    use crate::frontend;

    #[test]
    fn rejects_builtin_eval() {
        let program = frontend::parse("eval('1');").unwrap();
        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn rejects_non_literal_direct_eval() {
        let program = frontend::parse(
            r#"
            let source = "1";
            eval(source);
            "#,
        )
        .unwrap();

        let error = validate_refined_aot(&program).unwrap_err();
        assert!(error.to_string().contains("compile-time string literal"));
    }

    #[test]
    fn allows_direct_eval_comment_patterns() {
        let program = frontend::parse(
            r#"
            var xx = String.fromCharCode(0x000A);
            eval("//var " + xx + "yy = -1");
            eval("/*var " + String.fromCharCode(0x0000) + "xx = 1*/");
            "#,
        )
        .unwrap();

        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn allows_static_function_constructor_literal_sources() {
        let program = frontend::parse("new Function('value', 'return value + 1;');").unwrap();
        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn rejects_dynamic_function_constructor() {
        let program = frontend::parse(
            r#"
            let body = "return 1;";
            new Function(body);
            "#,
        )
        .unwrap();
        let error = validate_refined_aot(&program).unwrap_err();
        assert!(error.to_string().contains("runtime source evaluation"));
    }

    #[test]
    fn rejects_realm_eval() {
        let program = frontend::parse("Realm.eval('1');").unwrap();
        let error = validate_refined_aot(&program).unwrap_err();
        assert!(error.to_string().contains("runtime source evaluation"));
    }

    #[test]
    fn allows_shadowed_eval_binding() {
        let program = frontend::parse(
            r#"
            function eval(value) {
              return value;
            }

            console.log(eval(1));
            "#,
        )
        .unwrap();

        validate_refined_aot(&program).unwrap();
    }

    #[test]
    fn allows_outer_scope_eval_shadowing_for_nested_functions() {
        let program = frontend::parse(
            r#"
            function outer() {
              let eval = 1;

              function inner() {
                return eval;
              }

              return inner();
            }

            console.log(outer());
            "#,
        )
        .unwrap();

        validate_refined_aot(&program).unwrap();
    }
}
