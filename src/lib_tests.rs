use std::{fs, process::Command};

use crate::{
    CompileOptions, compile_file, compile_file_with_goal, compile_source,
    compile_source_with_goal, emit_wasm, frontend,
    ir::hir::{Expression, Statement, UpdateOp},
};

#[test]
fn emits_direct_wasm_bytes_for_supported_top_level_programs() {
    let program = frontend::parse(
        r#"
        let total = 0;
        let i = 1;

        while (i <= 3) {
          total = total + i;
          i = i + 1;
        }

        if (total === 6) {
          console.log("ok");
        } else {
          console.log("bad");
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support this subset");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_numeric_functions_and_for_loops() {
    let program = frontend::parse(
        r#"
        function sumTo(limit) {
          let total = 0;

          for (let i = 0; i <= limit; i++) {
            total = total + i;
          }

          return total;
        }

        let counter = 1;
        let before = counter++;
        let after = ++counter;

        console.log("sum", sumTo(5), before, after);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support numeric functions and for-loops");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_integer_exponentiation() {
    let program = frontend::parse(
        r#"
        let squared = 2 ** 3;
        let cube = 3 ** 0;

        console.log(squared, cube);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support integer exponentiation with numeric literal exponent");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_variable_exponentiation() {
    let program = frontend::parse(
        r#"
        let base = 2;
        let exp = 4;
        let power = base ** exp;

        console.log(power);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support variable exponentiation");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

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
                Statement::Var { name: first_name, .. },
                Statement::Assign {
                    name: first_assign_name,
                    value: Expression::Number(first_value),
                },
                Statement::Var { name: second_name, .. },
                Statement::Assign {
                    name: second_assign_name,
                    value: Expression::Number(second_value),
                },
                Statement::Expression(Expression::Identifier(name)),
                Statement::Expression(Expression::Update {
                    name: update_name,
                    op: UpdateOp::Increment,
                    prefix: true,
                }),
            ] if first_name == "x"
                && first_assign_name == "x"
                && *first_value == 0.0
                && second_name == "y"
                && second_assign_name == "y"
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
                Statement::Var { name: first_name, .. },
                Statement::Assign {
                    name: first_assign_name,
                    value: Expression::Number(first_value),
                },
                Statement::Var { name: second_name, .. },
                Statement::Assign {
                    name: second_assign_name,
                    value: Expression::Number(second_value),
                },
                Statement::Expression(Expression::Identifier(name)),
                Statement::Expression(Expression::Update {
                    name: update_name,
                    op: UpdateOp::Decrement,
                    prefix: true,
                }),
            ] if first_name == "x"
                && first_assign_name == "x"
                && *first_value == 1.0
                && second_name == "y"
                && second_assign_name == "y"
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
fn compile_options_are_constructible() {
    let options = CompileOptions {
        output: "out.wasm".into(),
        target: "wasm32-wasip2".to_string(),
    };

    assert_eq!(options.target, "wasm32-wasip2");
}

#[test]
fn compile_file_accepts_numeric_functions_on_the_direct_wasm_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir.path().join("numeric-functions.js");
    let output = tempdir.path().join("numeric-functions.wasm");

    fs::write(
        &input,
        r#"
        function sumTo(limit) {
          let total = 0;

          for (let i = 0; i <= limit; i++) {
            total = total + i;
          }

          return total;
        }

        let counter = 1;
        let before = counter++;
        let after = ++counter;

        console.log("sum", sumTo(5), before, after);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file(&input, &options).unwrap();
    assert!(output.exists());
}

#[test]
fn rejects_module_goal_sources_without_module_support() {
    let tempdir = tempfile::tempdir().unwrap();
    let output = tempdir.path().join("module.wasm");
    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    compile_source_with_goal(
        r#"
        let value = 1;
        console.log(value);
        "#,
        &options,
        true,
    )
    .expect_err("module goals are not yet supported by direct wasm backend");
}

#[test]
fn compile_file_uses_direct_wasm_backend_for_supported_programs() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir.path().join("direct-wasm.js");
    let output = tempdir.path().join("direct-wasm.wasm");

    fs::write(
        &input,
        r#"
        let total = 0;
        let i = 1;

        while (i <= 3) {
          total = total + i;
          i = i + 1;
        }

        if (total === 6) {
          console.log("ok");
        } else {
          console.log("bad");
        }
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file(&input, &options).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compile_file_executes_for_of_continue_via_direct_wasm_backend() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir.path().join("for-of-continue.js");
    let output = tempdir.path().join("for-of-continue.wasm");

    fs::write(
        &input,
        r#"
        let sum = 0;
        for (const value of [1, 2, 3]) {
          if (value === 2) {
            continue;
          }
          sum = sum + value;
        }
        console.log(sum);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file(&input, &options).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "4\n");
}

#[test]
fn compile_file_executes_nested_for_of_labeled_continue_outer_loop_via_direct_wasm_backend() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir.path().join("nested-for-of-labeled-continue.js");
    let output = tempdir.path().join("nested-for-of-labeled-continue.wasm");

    fs::write(
        &input,
        r#"
        let sum = 0;
        outer: for (const outer_value of [1, 2, 3]) {
          for (const inner_value of [4, 5, 6]) {
            if (inner_value === 5) {
              continue outer;
            }
            sum = sum + inner_value;
          }
        }
        console.log(sum);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file(&input, &options).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "12\n");
}

#[test]
fn compile_file_executes_nested_for_of_continue_outer_loop_closes_inner_only() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir
        .path()
        .join("nested-for-of-labeled-continue-inner-close.js");
    let output = tempdir
        .path()
        .join("nested-for-of-labeled-continue-inner-close.wasm");

    fs::write(
        &input,
        r#"
        function makeIterable(values, tracker) {
          let index = 0;
          const iterator = {
            next: function () {
              if (index >= values.length) {
                return { done: true };
              }
              return { value: values[index++], done: false };
            },
            return: function () {
              tracker.value = tracker.value + 1;
              return { done: true };
            },
          };

          iterator[Symbol.iterator] = function () {
            return iterator;
          };

          return iterator;
        }

        let outerClosed = { value: 0 };
        let innerClosed = { value: 0 };

        outer: for (const outerValue of makeIterable([1, 2], outerClosed)) {
          for (const innerValue of makeIterable([4, 5], innerClosed)) {
            if (innerValue === 5) {
              continue outer;
            }
            let skip = outerValue + innerValue;
          }
        }

        console.log(outerClosed.value, innerClosed.value);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file(&input, &options).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "0 2\n");
}

#[test]
fn compile_file_executes_nested_for_of_labeled_break_outer_loop_via_direct_wasm_backend() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir.path().join("nested-for-of-labeled-break.js");
    let output = tempdir.path().join("nested-for-of-labeled-break.wasm");

    fs::write(
        &input,
        r#"
        let sum = 0;
        outer: for (const outer_value of [1, 2, 3]) {
          for (const inner_value of [4, 5, 6]) {
            if (inner_value === 5) {
              break outer;
            }
            sum = sum + inner_value;
          }
        }
        console.log(sum);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file(&input, &options).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "4\n");
}

#[test]
fn compile_file_executes_labeled_for_of_current_loop_continue_closes_iterator() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir.path().join("for-of-continue-current-loop.js");
    let output = tempdir.path().join("for-of-continue-current-loop.wasm");

    fs::write(
        &input,
        r#"
        let sum = 0;
        let closed = false;
        const iterator = [1, 2, 3][Symbol.iterator]();

        iterator.return = function () {
          closed = true;
          return { done: true };
        };

        outer: for (const value of iterator) {
          if (value === 2) {
            continue outer;
          }
          sum = sum + value;
        }

        console.log(sum, closed ? 1 : 0);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file(&input, &options).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "4 1\n");
}

#[test]
fn compiles_module_goal_files_with_real_paths_fails_directly() {
    let tempdir = tempfile::tempdir().unwrap();
    let input = tempdir.path().join("module.js");
    let output = tempdir.path().join("module.wasm");
    fs::write(
        &input,
        r#"
        export const answer = 42;
        console.log(answer);
        "#,
    )
    .unwrap();
    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, true)
        .expect_err("module goals are not yet supported by direct wasm backend");
}

#[test]
fn rejects_named_and_namespace_module_imports() {
    let tempdir = tempfile::tempdir().unwrap();
    let dep = tempdir.path().join("dep.js");
    let reexport = tempdir.path().join("reexport.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &dep,
        r#"
        export const value = 7;
        export default function named() { return value; }
        "#,
    )
    .unwrap();
    fs::write(
        &reexport,
        r#"
        export * from "./dep.js";
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import named, { value } from "./dep.js";
        import * as ns from "./reexport.js";
        console.log(named(), value, ns.value, ns[Symbol.toStringTag]);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true)
        .expect_err("module imports are not yet supported by direct wasm backend");
}

#[test]
fn emits_direct_wasm_bytes_for_nullish_coalescing() {
    let program = frontend::parse(
        r#"
        let selected = undefined ?? 1;
        let explicit = null ?? 7;
        let keep = 0 ?? 5;

        console.log(selected, explicit, keep);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support nullish coalescing");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_void_unary() {
    let program = frontend::parse(
        r#"
        let counter = 0;
        let value = void 0;
        let side_effect = void (counter = 1);

        console.log(value, side_effect);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support unary void");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_printing_known_operands() {
    let program = frontend::parse(
        r#"
        console.log(typeof 12, typeof "text", typeof null, typeof undefined);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support typeof printing for known operand forms");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_numeric_unary_plus_from_numeric_string() {
    let program = frontend::parse(
        r#"
        let text = "12";
        let value = +text;

        console.log(value);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect(
            "direct wasm backend should support unary plus numeric coercion from parseable string literals",
        );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_of_tracked_identifiers() {
    let program = frontend::parse(
        r#"
        let text = "12";
        let value = +text;
        let none = undefined;

        console.log(typeof text, typeof value, typeof none);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should infer known identifier kinds for typeof printing");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_expression_comparisons() {
    let program = frontend::parse(
        r#"
        let is_number = typeof 12 === "number";
        let is_text = typeof "abc" === "string";
        let is_undefined = typeof undefined === "undefined";

        console.log(is_number, is_text, is_undefined);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support typeof comparisons in expression position");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_unknown_string_comparison() {
    let program = frontend::parse(
        r#"
        let is_never = typeof undefined === "not-a-type";
        let is_not = typeof [] !== "bogus-type";
        console.log(is_never, is_not);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should constant-fold unsupported typeof comparison strings",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_loose_typeof_unknown_string_comparison() {
    let program = frontend::parse(
        r#"
        let is_match = typeof 1 == "not-a-type";
        let is_not = typeof [] != "bad-type";
        console.log(is_match, is_not);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should fold loose typeof comparisons for unknown strings");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_unknown_comparison_with_side_effecting_operand() {
    let program = frontend::parse(
        r#"
        let value = 0;
        let _result = typeof (value = 7) === "impossible";
        console.log(value);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should preserve side effects before constant-folded typeof checks",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_of_composite_literals() {
    let program = frontend::parse(
        r#"
        console.log(typeof {} === "object", typeof [] === "object");
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support typeof on composite literals");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_of_undeclared_identifier() {
    let program = frontend::parse(
        r#"
        console.log(typeof definitely_undeclared === "undefined");
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support typeof for undeclared identifiers");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_of_global_intrinsics() {
    let program = frontend::parse(
        r#"
        console.log(
          typeof Number === "function",
          typeof Object === "function",
          typeof Math === "object",
          typeof Infinity === "number",
          typeof NaN === "number",
        );
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should classify common intrinsic globals for typeof");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_function_declaration() {
    let program = frontend::parse(
        r#"
        function tagged() {
          return 1;
        }

        console.log(typeof tagged === "function");
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should classify top-level function declarations for typeof",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_typeof_this() {
    let program = frontend::parse(
        r#"
        let this_kind = typeof this;
        console.log(this_kind === "object");
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support typeof this");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_builtin_function_calls() {
    let program = frontend::parse(
        r#"
        let number_tag = Number(12);
        let string_tag = String(12);
        let bool_tag = Boolean(0);
        let object_tag = Object({ foo: 1 });
        console.log(number_tag, string_tag, bool_tag, object_tag);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support conservative intrinsic function calls");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_builtin_function_calls_with_spread_args() {
    let program = frontend::parse(
        r#"
        let argument = [12];
        let number_tag = Number(...argument);
        let bool_tag = Boolean(...[0]);
        let object_tag = Object(...[{}]);
        console.log(number_tag, bool_tag, object_tag);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support conservative intrinsic function calls with spread-like args");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_user_function_calls_with_spread_args() {
    let program = frontend::parse(
        r#"
        function identity(value) {
          return value;
        }

        let argument = [42];
        console.log(identity(...argument));
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect(
            "direct wasm backend should support user function calls with spread-like arguments conservatively",
        );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_sequence_expressions() {
    let program = frontend::parse(
        r#"
        let base = 1;
        let value = (base = 2, base + 3);
        console.log(value);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support sequence expressions conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_update_expressions() {
    let program = frontend::parse(
        r#"
        let count = 0;
        count++;
        ++count;
        console.log(count);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support update expressions conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
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
fn emits_direct_wasm_bytes_for_new_target_expression() {
    let program = frontend::parse(
        r#"
        function probe() {
          return new.target;
        }

        let value = probe();
        console.log(value);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support new.target conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_delete_printing() {
    let program = frontend::parse(
        r#"
        let local = 1;
        console.log(delete 1, delete local, delete (local = 2));
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support delete printing for conservative cases");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_delete_in_expression_position() {
    let program = frontend::parse(
        r#"
        let local = 1;
        let deleted_local = delete local;
        let deleted_undeclared = delete definitely_undeclared;
        let deleted_expression = delete (local = 2);
        console.log(deleted_local, deleted_undeclared, deleted_expression);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support delete in expression position");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_delete_member_forms() {
    let program = frontend::parse(
        r#"
        let obj = 1;
        let deleted_member = delete (obj).value;
        console.log(delete obj.field, delete obj.value, deleted_member);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support delete on member forms conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_member_expressions() {
    let program = frontend::parse(
        r#"
        let obj = 1;
        let x = obj.value;
        let y = (obj = 2).field;
        let z = this;
        console.log(x, y, z);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should support member expressions with conservative lowering",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_member_assignment_expressions() {
    let program = frontend::parse(
        r#"
        let obj = 1;
        let assigned = (obj.value = (obj = 2));
        let chained = (obj.value = obj);
        console.log(assigned, chained);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should support member assignment expressions conservatively",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_member_assignment_statements() {
    let program = frontend::parse(
        r#"
        let obj = 1;
        obj.value = 2;
        obj.nested = obj;
        console.log(obj);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should support member assignment statements conservatively",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_with_statement() {
    let program = frontend::parse(
        r#"
        let obj = 1;
        with ({}) {
          let nested = 2;
          console.log(nested);
        }
        console.log(obj);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support with as conservative no-op scoping");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_throw_away_try_statement() {
    let program = frontend::parse(
        r#"
        let value = 0;
        try {
          value = 1;
        } catch (error) {
          let local = error;
          value = 2;
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should emit bytes for conservative try-catch lowering");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_switch_statement() {
    let program = frontend::parse(
        r#"
        let total = 0;
        let value = 2;
        switch (value) {
          case 1:
            total = 1;
            break;
          case 2:
            total = 2;
          default:
            total = 3;
        }
        console.log(total);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should emit bytes for switch statements conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_labeled_switch_with_labeled_break() {
    let program = frontend::parse(
        r#"
        let value = 0;
        let total = 0;
        outer: switch (value) {
          case 0:
            total = 1;
            break outer;
          case 1:
            total = 2;
        }
        console.log(total);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should emit bytes for labeled switch breaks");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_switch_with_unlabeled_break() {
    let program = frontend::parse(
        r#"
        let total = 0;
        let value = 2;
        switch (value) {
          case 1:
            total = 1;
            break;
          case 2:
            total = 2;
          default:
            total = 3;
        }
        console.log(total);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should emit bytes for switch statements with fallthrough breaks",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_labeled_statement_bodies() {
    let program = frontend::parse(
        r#"
        outer: {
          let value = 1;
          console.log(value);
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile labeled statement bodies conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_labeled_loop_with_continue() {
    let program = frontend::parse(
        r#"
        let counter = 0;
        outer: while (counter < 2) {
          counter = counter + 1;
          continue outer;
        }
        console.log(counter);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile labeled loop continues conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_labeled_loop_with_break() {
    let program = frontend::parse(
        r#"
        let counter = 0;
        outer: while (counter < 2) {
          break outer;
        }
        console.log(counter);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile labeled loop breaks conservatively");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_nested_switch_with_outer_loop_break() {
    let program = frontend::parse(
        r#"
        let value = 0;
        let counter = 0;
        outer: while (counter < 2) {
          switch (value) {
            case 0:
              counter = counter + 1;
              break outer;
            default:
              counter = 99;
          }
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile nested labeled break from switch to loop");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_nested_for_of_break_outer_loop() {
    let program = frontend::parse(
        r#"
        outer: for (const outer_value of [1, 2]) {
          for (const inner_value of [3, 4]) {
            break outer;
          }
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile nested for-of break to outer label");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_nested_for_of_continue_outer_loop() {
    let program = frontend::parse(
        r#"
        outer: for (const outer_value of [1, 2, 3]) {
          for (const inner_value of [4, 5]) {
            continue outer;
          }
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile nested for-of continue to outer label");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_for_of_loop() {
    let program = frontend::parse(
        r#"
        for (const value of [1, 2]) {
          console.log(value);
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should emit bytes for for-of with conservative iterator lowering",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_for_of_loop_with_break_hook() {
    let program = frontend::parse(
        r#"
        for (const value of [1, 2, 3]) {
          break;
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should emit bytes for for-of loops with break exits");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_for_of_loop_with_continue() {
    let program = frontend::parse(
        r#"
        for (const value of [1, 2, 3]) {
          if (value < 0) {
            continue;
          }
          console.log(value);
          continue;
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile for-of loops with continue exits");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_labeled_for_of_continue_current_loop() {
    let program = frontend::parse(
        r#"
        let closed = false;
        const iterator = [1, 2, 3][Symbol.iterator]();
        iterator.return = function () {
          closed = true;
          return { done: true };
        };

        outer: for (const value of iterator) {
          if (value === 2) {
            continue outer;
          }
          console.log(value);
        }
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should compile labeled for-of continue to current loop");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn rejects_generator_functions_from_direct_backend_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let output = tempdir.path().join("generator.wasm");
    let options = CompileOptions {
        output: output,
        target: "wasm32-wasip2".to_string(),
    };

    let result = compile_source("function* generator() { yield 1; }", &options);

    assert!(result.is_err());
    assert!(matches!(
        result.err().map(|error| error
            .to_string()
            .contains("not yet supported by the direct wasm backend")),
        Some(true)
    ));
}

#[test]
fn emits_direct_wasm_bytes_for_throw_statement() {
    let program = frontend::parse(
        r#"
        let tag = 1;
        throw tag;
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should emit wasm bytes for throw as unreachable");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_new_expressions() {
    let program = frontend::parse(
        r#"
        let n = 0;
        let tag = new Number((n = 1));
        let boxed = new Object();
        console.log(tag, boxed, n);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should support new expressions with conservative lowering",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_in_operator() {
    let program = frontend::parse(
        r#"
        let container = 0;
        let in_left = "text" in container;
        let in_side_effect = ("k") in (container = 1);
        let in_function = "length" in Object;
        let in_object = "size" in Math;
        let in_literal = "x" in {};
        console.log(in_left, in_side_effect, in_function, in_object, in_literal);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program)
        .unwrap()
        .expect("direct wasm backend should support in operator with conservative value");

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn emits_direct_wasm_bytes_for_instanceof_operator() {
    let program = frontend::parse(
        r#"
        let value = 1;
        let constructor = 0;
        let first = value instanceof constructor;
        let second = (value = 2) instanceof constructor;
        let third = value instanceof Number;
        let fourth = (value = 3) instanceof Object;
        let fifth = 4 instanceof {};
        console.log(first, second, third, fourth, fifth);
        "#,
    )
    .unwrap();

    let wasm = emit_wasm(&program).unwrap().expect(
        "direct wasm backend should support instanceof operator with conservative value",
    );

    assert!(wasm.starts_with(b"\0asm\x01\0\0\0"));
}

#[test]
fn rejects_number_literals_colliding_with_js_nullish_tags_in_direct_backend() {
    let program = frontend::parse(r#"let value = -1073741824;"#).unwrap();
    let wasm = emit_wasm(&program).unwrap();
    assert!(
        wasm.is_none(),
        "direct backend must reject number literals that collide with reserved nullish tags"
    );
}

#[test]
fn rejects_string_literals_that_coerce_to_reserved_js_tags_in_loose_comparisons() {
    let program = frontend::parse(r#"let eq = "-1073741823" == 0;"#).unwrap();
    let wasm = emit_wasm(&program).unwrap();
    assert!(
        wasm.is_none(),
        "direct backend must reject loose string comparisons that collide with reserved nullish tags"
    );
}

#[test]
fn rejects_number_literals_colliding_with_js_typeof_tags_in_direct_backend() {
    let program = frontend::parse(r#"let value = -1073741822;"#).unwrap();
    let wasm = emit_wasm(&program).unwrap();
    assert!(
        wasm.is_none(),
        "direct backend must reject numeric literals that collide with internal typeof tags"
    );
}
