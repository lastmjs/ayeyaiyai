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

#[test]
fn rejects_invalid_numeric_separator_placements() {
    let invalid_sources = [
        "0b_1", "0x_FF", "1__0", "1_.0", "1._0", "1e_1", "1e+_1", "0_1", "0_1.5",
    ];

    for source in invalid_sources {
        assert!(
            frontend::validate_script_goal(source).is_err(),
            "source should fail to parse:\n{source}"
        );
    }
}

#[test]
fn accepts_valid_numeric_separator_placements() {
    let valid_sources = ["0b1_0", "0xA_B", "1_0", "1_0.5_0", "1.0_5e+1_0"];

    for source in valid_sources {
        frontend::validate_script_goal(source).expect("source should parse");
    }
}

#[test]
fn rejects_escaped_reserved_words_in_binding_identifiers() {
    let invalid_sources = [
        "var \\u{65}lse = 123;",
        "var \\u0065lse = 123;",
        "var \\u{64}elete = 123;",
        "var \\u0064elete = 123;",
        "var \\u{65}\\u{6e}\\u{75}\\u{6d} = 123;",
        "var \\u0065\\u006e\\u0075\\u006d = 123;",
    ];

    for source in invalid_sources {
        assert!(
            frontend::validate_script_goal(source).is_err(),
            "source should fail to parse:\n{source}"
        );
    }
}

#[test]
fn parse_script_goal_rejects_escaped_await_binding_in_async_generator_method() {
    let source = r#"
    class C { async *gen() {
        var \u0061wait;
    }}
    "#;

    assert!(
        frontend::parse_script_goal(source).is_err(),
        "source should fail to parse:\n{source}"
    );
}

#[test]
fn parse_script_goal_rejects_await_binding_in_async_generator_method() {
    let source = r#"
    class C { async *gen() {
        var await;
    }}
    "#;

    assert!(
        frontend::parse_script_goal(source).is_err(),
        "source should fail to parse:\n{source}"
    );
}

#[test]
fn parse_script_goal_rejects_await_identifier_reference_in_async_generator_method() {
    let source = r#"
    class C { async *gen() {
        await;
    }}
    "#;

    assert!(
        frontend::parse_script_goal(source).is_err(),
        "source should fail to parse:\n{source}"
    );
}

#[test]
fn parse_script_goal_rejects_await_label_in_async_generator_method() {
    let source = r#"
    class C { async *gen() {
        await: 1;
    }}
    "#;

    assert!(
        frontend::parse_script_goal(source).is_err(),
        "source should fail to parse:\n{source}"
    );
}

#[test]
fn parse_script_goal_accepts_decorator_member_private_identifier_in_static_block() {
    let source = r#"
    class C {
      static #yield() {}
      static #await() {}
      static {
        @C.#yield
        @C.#await
        class D {}
      }
    }
    "#;

    frontend::parse_script_goal(source).expect("source should parse");
}

#[test]
fn parse_script_goal_accepts_nested_yield_spread_in_async_generator_method() {
    let valid_sources = [
        r#"
        class C { async *gen() {
            yield [...yield yield];
        }}
        "#,
        r#"
        class C { async *gen() {
            yield [...yield];
        }}
        "#,
        r#"
        class C { async *gen() {
            yield {...yield};
        }}
        "#,
    ];

    for source in valid_sources {
        assert!(
            frontend::parse_script_goal(source).is_ok(),
            "source should parse:\n{source}"
        );
    }
}

#[test]
fn accepts_escaped_non_reserved_binding_identifiers() {
    let valid_sources = [
        "var \\u{65}lsewhere = 123;",
        "var $\\u200D = 2;",
        "var $\\u200C = 3;",
    ];

    for source in valid_sources {
        frontend::validate_script_goal(source)
            .expect("non-reserved escaped identifier should parse");
    }
}

#[test]
fn parse_script_goal_accepts_escaped_await_class_name_identifier() {
    let source = r#"
    class aw\u0061it {}
    "#;

    frontend::parse_script_goal(source)
        .expect("escaped await class name should parse in script goal");
}

#[test]
fn parse_script_goal_rejects_duplicate_parameters_in_async_class_method() {
    let source = r#"
    class Foo {
      async foo(a, a) {}
    }
    "#;

    assert!(
        frontend::validate_script_goal(source).is_err(),
        "duplicate parameters in async class methods should be rejected"
    );
}

#[test]
fn rejects_invalid_escaped_identifier_starts_and_code_points() {
    let invalid_sources = ["var \\u200D;", "var \\u200C;", "var \\u{00_76} = 1;"];

    for source in invalid_sources {
        assert!(
            frontend::validate_script_goal(source).is_err(),
            "source should fail to parse:\n{source}"
        );
    }
}
