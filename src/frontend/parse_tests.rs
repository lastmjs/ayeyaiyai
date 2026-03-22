use super::parse;
use crate::{
    frontend,
    ir::hir::{Expression, Statement, UpdateOp},
};

#[test]
fn parses_asi_prefix_increment_as_separate_expression_statements() {
    let program = frontend::parse(
        r#"
        var x = 0;
        var y = 0;
        x
        ++y
        "#,
    )
    .unwrap();

    assert!(
        matches!(
            program.statements.as_slice(),
            [
                Statement::Var {
                    name: first_name,
                    value: Expression::Number(first_value),
                },
                Statement::Var {
                    name: second_name,
                    value: Expression::Number(second_value),
                },
                Statement::Expression(Expression::Identifier(name)),
                Statement::Expression(Expression::Update {
                    name: update_name,
                    op: UpdateOp::Increment,
                    prefix: true,
                }),
            ] if first_name == "x"
                && *first_value == 0.0
                && second_name == "y"
                && *second_value == 0.0
                && name == "x"
                && update_name == "y"
        ),
        "{:#?}",
        program.statements
    );
}

#[test]
fn parses_asi_prefix_decrement_as_separate_expression_statements() {
    let program = frontend::parse(
        r#"
        var x = 1;
        var y = 1;
        x
        --y
        "#,
    )
    .unwrap();

    assert!(
        matches!(
            program.statements.as_slice(),
            [
                Statement::Var {
                    name: first_name,
                    value: Expression::Number(first_value),
                },
                Statement::Var {
                    name: second_name,
                    value: Expression::Number(second_value),
                },
                Statement::Expression(Expression::Identifier(name)),
                Statement::Expression(Expression::Update {
                    name: update_name,
                    op: UpdateOp::Decrement,
                    prefix: true,
                }),
            ] if first_name == "x"
                && *first_value == 1.0
                && second_name == "y"
                && *second_value == 1.0
                && name == "x"
                && update_name == "y"
        ),
        "{:#?}",
        program.statements
    );
}

#[test]
fn rejects_classic_for_headers_with_only_one_semicolon() {
    let invalid_sources = [
        "for(false;false\n) { break; }",
        "for(false;\nfalse\n) { break; }",
        "for(false\n    ;\n) { break; }",
        "for(false\n    ;false\n) { break; }",
        "for(\n;false) { break; }",
    ];

    for source in invalid_sources {
        assert!(
            frontend::validate_script_goal(source).is_err(),
            "source should fail to parse:\n{source}"
        );
    }
}

#[test]
fn accepts_classic_for_headers_with_two_semicolons_across_newlines() {
    let source = r#"
    for(false
        ;false
        ;
    ) {
      break;
    }
    "#;

    frontend::validate_script_goal(source).expect("source should parse");
}

#[test]
fn parses_top_level_global_this_update_as_binding_update() {
    let program = frontend::parse(
        r#"
        var y;
        this.y++;
        "#,
    )
    .unwrap();

    assert!(
        matches!(
            program.statements.as_slice(),
            [
                Statement::Var { name, value },
                Statement::Expression(Expression::Update {
                    name: update_name,
                    op: UpdateOp::Increment,
                    prefix: false,
                }),
            ] if name == "y"
                && matches!(value, Expression::Undefined)
                && update_name == "y"
        ),
        "{:#?}",
        program.statements
    );
}

#[test]
fn parses_hashbang_comments_terminated_by_carriage_return() {
    parse("#! comment\r{}\n").expect("carriage-return-terminated hashbang should parse");
}

#[test]
fn parses_hashbang_comments_terminated_by_line_separator() {
    parse("#! comment\u{2028}{}\n").expect("line-separator-terminated hashbang should parse");
}

#[test]
fn parses_hashbang_comments_terminated_by_paragraph_separator() {
    parse("#! comment\u{2029}{}\n").expect("paragraph-separator-terminated hashbang should parse");
}
