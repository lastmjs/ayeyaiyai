use std::{fs, path::Path, process::Command};

use ayeyaiyai::{CompileOptions, compile_file, compile_file_with_goal, compile_source_with_goal};
use tempfile::tempdir;

fn assert_cli_compile_rejected(input: &Path, output: &Path, expected: &str) {
    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(input)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap();

    assert!(
        !compile.status.success(),
        "compiler unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let stderr = String::from_utf8_lossy(&compile.stderr);
    assert!(stderr.contains(expected), "{stderr}");
}

#[test]
fn compiles_and_runs_a_small_program_with_wasmtime() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("sum.js");
    let output = tempdir.path().join("sum.wasm");

    fs::write(
        &input,
        r#"
        let total = 0;
        let i = 1;

        while (i <= 5) {
          total = total + i;
          i = i + 1;
        }

        if (total === 15) {
          console.log("sum", total);
        } else {
          console.log("unexpected", total);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "sum 15\n");
}

#[test]
fn compiles_functions_returns_and_short_circuiting() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("functions.js");
    let output = tempdir.path().join("functions.wasm");

    fs::write(
        &input,
        r#"
        function add(a, b) {
          return a + b;
        }

        function fallback(left, right) {
          return left || right;
        }

        function select(left, right) {
          return left ?? right;
        }

        let total = add(4, 5);
        let empty = "";
        let chosen = select(undefined, "set");
        let gated = true && "yes";

        console.log("values", total, fallback(empty, "fallback"), chosen, gated);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "values 9 fallback set yes\n"
    );
}

#[test]
fn compiles_nested_for_of_labeled_continue_outer_loop() {
    let tempdir = tempdir().unwrap();
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

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "12\n");
}

#[test]
fn compiles_nested_for_of_continue_outer_loop_closes_inner_only() {
    let tempdir = tempdir().unwrap();
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

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "0 2\n");
}

#[test]
fn compiles_async_generator_yield_star_sync_next_with_getter_returned_capture_slots() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-yield-star-sync-next.js");
    let output = tempdir
        .path()
        .join("async-generator-yield-star-sync-next.wasm");

    fs::write(
        &input,
        r#"
        var obj = {
          get [Symbol.iterator]() {
            return function() {
              var log = [];
              var nextCount = 0;
              return {
                name: "syncIterator",
                get next() {
                  return function() {
                    log.push(arguments.length);
                    nextCount++;
                    if (nextCount === 1) {
                      return {
                        value: "next-value-1",
                        done: false,
                      };
                    }
                    return {
                      value: "next-value-2",
                      done: true,
                    };
                  };
                },
              };
            };
          },
          get [Symbol.asyncIterator]() {
            return null;
          },
        };

        class C {
          static async *gen() {
            var value = yield* obj;
            return value;
          }
        }

        var iter = C.gen();
        iter.next("first").then(first => {
          iter.next("second").then(second => {
            console.log(first.value, second.value);
          });
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "next-value-1 next-value-2\n"
    );
}

#[test]
fn compiles_unbound_async_generator_yield_star_sync_next_sequence() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-yield-star-sync-next-unbound.js");
    let output = tempdir
        .path()
        .join("async-generator-yield-star-sync-next-unbound.wasm");

    fs::write(
        &input,
        r#"
        var log = [];
        var obj = {
          get [Symbol.iterator]() {
            log.push({ name: "get [Symbol.iterator]", thisValue: this });
            return function() {
              log.push({ name: "call [Symbol.iterator]", thisValue: this, args: [...arguments] });
              var nextCount = 0;
              return {
                name: "syncIterator",
                get next() {
                  log.push({ name: "get next", thisValue: this });
                  return function() {
                    log.push({ name: "call next", thisValue: this, args: [...arguments] });
                    nextCount++;
                    if (nextCount == 1) {
                      return {
                        name: "next-result-1",
                        get value() { log.push({ name: "get next value (1)", thisValue: this }); return "next-value-1"; },
                        get done() { log.push({ name: "get next done (1)", thisValue: this }); return false; }
                      };
                    }
                    return {
                      name: "next-result-2",
                      get value() { log.push({ name: "get next value (2)", thisValue: this }); return "next-value-2"; },
                      get done() { log.push({ name: "get next done (2)", thisValue: this }); return true; }
                    };
                  };
                }
              };
            };
          },
          get [Symbol.asyncIterator]() {
            log.push({ name: "get [Symbol.asyncIterator]" });
            return null;
          }
        };

        class C {
          async *gen() {
            log.push({ name: "before yield*" });
            var value = yield* obj;
            log.push({ name: "after yield*", value: value });
            return "return-value";
          }
        }

        var gen = C.prototype.gen;
        var iter = gen();
        iter.next("first").then(first => {
          return iter.next("second").then(second => {
            console.log(
              first.value,
              second.value,
              second.done,
              log.map(entry => entry.name).join("|")
            );
          });
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "next-value-1 return-value true before yield*|get [Symbol.asyncIterator]|get [Symbol.iterator]|call [Symbol.iterator]|get next|call next|get next done (1)|get next value (1)|call next|get next done (2)|get next value (2)|after yield*\n"
    );
}

#[test]
fn compiles_for_of_over_custom_iterator_breaks_and_closes() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("for-of-custom-iterator-break-close.js");
    let output = tempdir
        .path()
        .join("for-of-custom-iterator-break-close.wasm");

    fs::write(
        &input,
        r#"
        let closed = 0;

        function makeIterable(values) {
          let index = 0;
          const iterator = {
            next: function () {
              if (index >= values.length) {
                return { done: true };
              }
              return { value: values[index++], done: false };
            },
            return: function () {
              closed = closed + 1;
              return { done: true };
            },
          };

          iterator[Symbol.iterator] = function () {
            return iterator;
          };

          return iterator;
        }

        let count = 0;
        for (const value of makeIterable([4, 5])) {
          count = count + 1;
          if (value === 5) {
            break;
          }
        }

        console.log(count, closed);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "2 1\n");
}

#[test]
fn compiles_nested_for_of_labeled_break_outer_loop() {
    let tempdir = tempdir().unwrap();
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

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "4\n");
}

#[test]
fn compiles_labeled_for_of_current_loop_continue_closes_iterator() {
    let tempdir = tempdir().unwrap();
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

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "4 1\n");
}

#[test]
fn compiles_for_loops_and_update_expressions() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("for-loop.js");
    let output = tempdir.path().join("for-loop.wasm");

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

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "sum 15 1 3\n");
}

#[test]
fn compiles_top_level_global_this_property_updates() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("global-this-update.js");
    let output = tempdir.path().join("global-this-update.wasm");

    fs::write(
        &input,
        r#"
        var y;
        this.y++;
        console.log(isNaN(y));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n");
}

#[test]
fn compiles_ternaries_and_compound_assignment() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("ternary.js");
    let output = tempdir.path().join("ternary.wasm");

    fs::write(
        &input,
        r#"
        let total = 1;
        total += 4;
        total *= 2;

        let label = false ? "bad" : "good";
        let empty = "";
        empty ||= "fallback";

        console.log(label, total, empty);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "good 10 fallback\n");
}

#[test]
fn strict_direct_eval_rejects_arguments_assignment_in_nested_function() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("strict-eval.js");
    let output = tempdir.path().join("strict-eval.wasm");

    fs::write(
        &input,
        r#"
        "use strict";

        try {
          eval("(function inner() { arguments = 10; }());");
          console.log("unexpected");
        } catch (error) {
          console.log(error.name);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "SyntaxError\n");
}

#[test]
fn strict_direct_eval_reserved_word_parse_errors_throw_syntax_error_instances() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("strict-eval-reserved-word.js");
    let output = tempdir.path().join("strict-eval-reserved-word.wasm");

    fs::write(
        &input,
        r#"
        var err = null;

        try {
          eval("'use strict'; var public = 1; var anotherVariableNotReserveWord = 2;");
        } catch (e) {
          err = e;
        }

        console.log(
          err instanceof SyntaxError,
          typeof public,
          typeof anotherVariableNotReserveWord
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "true undefined undefined\n"
    );
}

#[test]
fn static_function_constructors_in_strict_code_do_not_inherit_strict_mode() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("function-constructor-strict.js");
    let output = tempdir.path().join("function-constructor-strict.wasm");

    fs::write(
        &input,
        r#"
        function testcase() {
          "use strict";
          var funObj = new Function("var public = 1; return 1;");
          console.log(funObj());
        }

        testcase();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1\n");
}

#[test]
fn arguments_length_is_writable_and_configurable() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arguments-length.js");
    let output = tempdir.path().join("arguments-length.wasm");

    fs::write(
        &input,
        r#"
        function testcase() {
          var writable = false;
          var configurable = false;

          arguments.length = "updated";
          writable = arguments.length === "updated";
          configurable = delete arguments.length;

          console.log(writable, configurable, Object.prototype.hasOwnProperty.call(arguments, "length"));
        }

        testcase(1, 2);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true true false\n");
}

#[test]
fn arguments_inherit_constructor_from_object_prototype() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arguments-constructor.js");
    let output = tempdir.path().join("arguments-constructor.wasm");

    fs::write(
        &input,
        r#"
        function f() {
          console.log(
            arguments.constructor === Object,
            arguments.constructor.prototype === Object.prototype
          );
        }

        f();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true true\n");
}

#[test]
fn arguments_callee_is_not_enumerable() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arguments-callee.js");
    let output = tempdir.path().join("arguments-callee.wasm");

    fs::write(
        &input,
        r#"
        function f() {
          var sawCallee = false;
          for (var key in arguments) {
            if (key === "callee") {
              sawCallee = true;
            }
          }
          console.log(sawCallee);
        }

        f(1);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "false\n");
}

#[test]
fn compiles_break_and_continue_in_loops() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("break-continue.js");
    let output = tempdir.path().join("break-continue.wasm");

    fs::write(
        &input,
        r#"
        let total = 0;

        for (let i = 0; i < 10; i++) {
          if (i === 2) {
            continue;
          }

          if (i === 5) {
            break;
          }

          total += i;
        }

        console.log("loop", total);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "loop 8\n");
}

#[test]
fn compiles_arrays_objects_and_member_access() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("objects.js");
    let output = tempdir.path().join("objects.wasm");

    fs::write(
        &input,
        r#"
        let values = [1, 2, 3];
        let person = {
          name: "Aye",
          score: values[1] + values[2],
          ok: true,
        };

        console.log(person.name, person["score"], values.length, "hello"[1]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Aye 5 3 e\n");
}

#[test]
fn compiles_static_string_length_for_noncharacter_code_points() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("string-length-noncharacter.js");
    let output = tempdir.path().join("string-length-noncharacter.wasm");

    fs::write(
        &input,
        r#"
        var prop = "a\uFFFFa";
        console.log(prop.length, prop[1] === "\uFFFF", prop !== "aa");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "3 true true\n");
}

#[test]
fn compiles_array_literal_instanceof_array_for_sparse_and_nested_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("array-instanceof.js");
    let output = tempdir.path().join("array-instanceof.wasm");

    fs::write(
        &input,
        r#"
        let empty = [];
        let sparse = [,,3,,,];
        let nested = [[1, 2], [3], []];
        let subarray = nested[0];

        console.log(
          empty instanceof Array,
          sparse instanceof Array,
          nested instanceof Array,
          subarray instanceof Array,
          subarray.length,
          subarray[0],
          subarray[1],
          sparse.length,
          sparse[2],
          sparse[4] === undefined,
          nested[1][0],
          nested[2] instanceof Array
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "true true true true 2 1 2 5 3 true 3 true\n"
    );
}

#[test]
fn compiles_member_assignment_for_objects_and_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("member-assign.js");
    let output = tempdir.path().join("member-assign.wasm");

    fs::write(
        &input,
        r#"
        let values = [1, 2];
        values[2] = 4;
        values[1] += 3;

        let person = { name: "Aye" };
        person.name = "Yai";
        person.score = values[1];

        console.log(person.name, person.score, values.length, values[2]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "Yai 5 3 4\n");
}

#[test]
fn compiles_computed_numeric_object_keys_and_own_property_name_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("computed-object-keys.js");
    let output = tempdir.path().join("computed-object-keys.wasm");

    fs::write(
        &input,
        r#"
        function __sameValue(a, b) {
          return a === b;
        }

        function compareArray(actual, expected) {
          if (actual.length !== expected.length) {
            return false;
          }
          for (var i = 0; i < actual.length; i = i + 1) {
            if (!__sameValue(actual[i], expected[i])) {
              return false;
            }
          }
          return true;
        }

        var assert = {};
        assert.sameValue = function(actual, expected) {
          if (!__sameValue(actual, expected)) {
            throw 2;
          }
        };
        assert.compareArray = function(actual, expected) {
          if (!compareArray(actual, expected)) {
            throw 1;
          }
        };

        function ID(x) {
          return x;
        }

        var object = {
          a: "A",
          [1]: "B",
          c: "C",
          [ID(2)]: "D",
        };

        assert.compareArray(Object.getOwnPropertyNames(object), ["1", "2", "a", "c"]);
        console.log("computed", object.a, object[1], object.c, object[2]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "computed A B C D\n");
}

#[test]
fn compiles_computed_symbol_object_keys_and_own_property_symbol_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("computed-symbol-object-keys.js");
    let output = tempdir.path().join("computed-symbol-object-keys.wasm");

    fs::write(
        &input,
        r#"
        function __sameValue(a, b) {
          return a === b;
        }

        function compareArray(actual, expected) {
          if (actual.length !== expected.length) {
            return false;
          }
          for (var i = 0; i < actual.length; i = i + 1) {
            if (!__sameValue(actual[i], expected[i])) {
              return false;
            }
          }
          return true;
        }

        var assert = {};
        assert.compareArray = function(actual, expected) {
          if (!compareArray(actual, expected)) {
            throw 1;
          }
        };

        function ID(x) {
          return x;
        }

        var sym1 = Symbol();
        var sym2 = Symbol();
        var object = {
          a: "A",
          [sym1]: "B",
          c: "C",
          [ID(sym2)]: "D",
        };

        assert.compareArray(Object.getOwnPropertyNames(object), ["a", "c"]);
        assert.compareArray(Object.getOwnPropertySymbols(object), [sym1, sym2]);
        console.log("symbols", object.a, object[sym1], object.c, object[sym2]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "symbols A B C D\n");
}

#[test]
fn compiles_computed_property_name_coercion_side_effects_for_objects_and_classes() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("computed-property-name-coercion-side-effects.js");
    let output = tempdir
        .path()
        .join("computed-property-name-coercion-side-effects.wasm");

    fs::write(
        &input,
        r#"
        function __sameValue(a, b) {
          return a === b;
        }

        function compareArray(actual, expected) {
          if (actual.length !== expected.length) {
            return false;
          }
          for (var i = 0; i < actual.length; i = i + 1) {
            if (!__sameValue(actual[i], expected[i])) {
              return false;
            }
          }
          return true;
        }

        var counter = 0;
        var objectStringKeyCalls = [];
        var objectNumberKeyCalls = [];
        var classStringKeyCalls = [];
        var classNumberKeyCalls = [];

        var objectStringKey = {
          toString: function() {
            objectStringKeyCalls.push(counter);
            counter += 1;
            return "b";
          }
        };

        var objectNumberKey = {
          valueOf: function() {
            objectNumberKeyCalls.push(counter);
            counter += 1;
            return 1;
          },
          toString: null
        };

        var classStringKey = {
          toString: function() {
            classStringKeyCalls.push(counter);
            counter += 1;
            return "b";
          }
        };

        var classNumberKey = {
          valueOf: function() {
            classNumberKeyCalls.push(counter);
            counter += 1;
            return 1;
          },
          toString: null
        };

        var object = {
          a: "A",
          [objectStringKey]: "B",
          c: "C",
          [objectNumberKey]: "D",
        };

        class C {
          a() { return "A"; }
          [classStringKey]() { return "B"; }
          c() { return "C"; }
          [classNumberKey]() { return "D"; }
        }

        if (!compareArray(objectStringKeyCalls, [0])) throw 1;
        if (!compareArray(objectNumberKeyCalls, [1])) throw 2;
        if (!compareArray(classStringKeyCalls, [2])) throw 3;
        if (!compareArray(classNumberKeyCalls, [3])) throw 4;
        if (counter !== 4) throw 5;

        if (object.a !== "A" || object.b !== "B" || object.c !== "C" || object[1] !== "D") throw 6;
        if (!compareArray(Object.getOwnPropertyNames(object), ["1", "a", "b", "c"])) throw 7;

        if (new C().a() !== "A") throw 8;
        if (new C().b() !== "B") throw 9;
        if (new C().c() !== "C") throw 10;
        if (new C()[1]() !== "D") throw 11;
        if (Object.keys(C.prototype).length !== 0) throw 12;
        if (!compareArray(Object.getOwnPropertyNames(C.prototype), ["1", "constructor", "a", "b", "c"])) throw 13;

        console.log("computed-side-effects");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "computed-side-effects\n"
    );
}

#[test]
fn compiles_duplicate_computed_class_getters_with_last_definition_winning() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("computed-class-getters.js");
    let output = tempdir.path().join("computed-class-getters.wasm");

    fs::write(
        &input,
        r#"
        class C {
          get ['a']() {
            return 'A';
          }
        }

        class C2 {
          get b() {
            throw 1;
          }
          get ['b']() {
            return 'B';
          }
        }

        class C3 {
          get c() {
            throw 2;
          }
          get ['c']() {
            throw 3;
          }
          get ['c']() {
            return 'C';
          }
        }

        class C4 {
          get ['d']() {
            throw 4;
          }
          get d() {
            return 'D';
          }
        }

        if (new C().a !== 'A') throw 5;
        if (new C2().b !== 'B') throw 6;
        if (new C3().c !== 'C') throw 7;
        if (new C4().d !== 'D') throw 8;
        console.log("class-getters");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "class-getters\n");
}

#[test]
fn compiles_computed_class_generator_method_prototype_name_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("computed-class-generator-method.js");
    let output = tempdir.path().join("computed-class-generator-method.wasm");

    fs::write(
        &input,
        r#"
        function __sameValue(a, b) {
          return a === b;
        }

        function compareArray(actual, expected) {
          if (actual.length !== expected.length) {
            return false;
          }
          for (var i = 0; i < actual.length; i = i + 1) {
            if (!__sameValue(actual[i], expected[i])) {
              return false;
            }
          }
          return true;
        }

        var assert = {};
        assert.sameValue = function(actual, expected) {
          if (!__sameValue(actual, expected)) {
            throw 1;
          }
        };
        assert.compareArray = function(actual, expected) {
          if (!compareArray(actual, expected)) {
            throw 2;
          }
        };

        class C {
          *['a']() {
            yield 1;
            yield 2;
          }
        }

        assert.sameValue(Object.keys(C.prototype).length, 0);
        assert.compareArray(Object.getOwnPropertyNames(C.prototype), ['constructor', 'a']);
        console.log("class-generator");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "class-generator\n");
}

#[test]
fn compiles_static_computed_class_prototype_methods_throw_type_error() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("static-computed-class-prototype-methods.js");
    let output = tempdir
        .path()
        .join("static-computed-class-prototype-methods.wasm");

    fs::write(
        &input,
        r#"
        __ayyAssertThrows(TypeError, function() {
          class C {
            static ['prototype']() {}
          }
        });

        __ayyAssertThrows(TypeError, function() {
          class C {
            static get ['prototype']() {
              return 1;
            }
          }
        });

        __ayyAssertThrows(TypeError, function() {
          class C {
            static set ['prototype'](x) {}
          }
        });

        __ayyAssertThrows(TypeError, function() {
          class C {
            static *['prototype']() {}
          }
        });

        console.log("class-static-prototype");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "class-static-prototype\n"
    );
}

#[test]
fn compiles_static_computed_class_symbol_method_order() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("static-computed-class-symbol-order.js");
    let output = tempdir
        .path()
        .join("static-computed-class-symbol-order.wasm");

    fs::write(
        &input,
        r#"
        function __sameValue(a, b) {
          return a === b;
        }

        function compareArray(actual, expected) {
          if (actual.length !== expected.length) {
            return false;
          }
          for (var i = 0; i < actual.length; i = i + 1) {
            if (!__sameValue(actual[i], expected[i])) {
              return false;
            }
          }
          return true;
        }

        var assert = {};
        assert.compareArray = function(actual, expected) {
          if (!compareArray(actual, expected)) {
            throw 1;
          }
        };

        var sym1 = Symbol();
        var sym2 = Symbol();
        class C {
          static a() { return "A"; }
          static [sym1]() { return "B"; }
          static c() { return "C"; }
          static [sym2]() { return "D"; }
        }

        assert.compareArray(Object.getOwnPropertyNames(C), ["length", "name", "prototype", "a", "c"]);
        assert.compareArray(Object.getOwnPropertySymbols(C), [sym1, sym2]);
        assert.sameValue(C.a(), "A");
        assert.sameValue(C[sym1](), "B");
        assert.sameValue(C.c(), "C");
        assert.sameValue(C[sym2](), "D");
        console.log("class-static-symbol");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "class-static-symbol\n"
    );
}

#[test]
fn compiles_computed_object_symbol_getters() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("computed-object-symbol-getters.js");
    let output = tempdir.path().join("computed-object-symbol-getters.wasm");

    fs::write(
        &input,
        r#"
        var s = Symbol();
        var A = {
          get ["a"]() {
            return "A";
          },
          get [1]() {
            return 1;
          },
          get [s]() {
            return s;
          }
        };

        if (A.a !== "A") {
          throw 1;
        }
        if (A[1] !== 1) {
          throw 1;
        }
        if (A[s] !== s) {
          throw 1;
        }

        console.log("object-symbol-getter");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "object-symbol-getter\n"
    );
}

#[test]
fn compiles_define_property_and_literal_getters_with_strict_reference_errors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("strict-getters.js");
    let output = tempdir.path().join("strict-getters.wasm");

    fs::write(
        &input,
        r#"
        __ayyAssertThrows(ReferenceError, function() {
          var obj = {};
          Object.defineProperty(obj, "accProperty", {
            get: function () {
              "use strict";
              test262unresolvable = null;
              return 11;
            }
          });
          obj.accProperty;
        }, "defineProperty-own");

        __ayyAssertThrows(ReferenceError, function() {
          "use strict";
          var obj = {};
          Object.defineProperty(obj, "accProperty", {
            get: function () {
              test262unresolvable = null;
              return 11;
            }
          });
          obj.accProperty;
        }, "defineProperty-inherited");

        __ayyAssertThrows(ReferenceError, function() {
          var obj = {
            get accProperty() {
              "use strict";
              test262literal = null;
              return 11;
            }
          };
          obj.accProperty;
        }, "literal-own");

        console.log("strict-getters");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "strict-getters\n");
}

#[test]
fn compiles_typeof_void_and_unary_plus() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("unary.js");
    let output = tempdir.path().join("unary.wasm");

    fs::write(
        &input,
        r#"
        let text = "12";
        let amount = +text;
        let nothing = void amount;

        console.log(typeof amount, amount, typeof nothing, typeof null, typeof Math, typeof Math.PI, typeof Math.exp, typeof Date);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "number 12 undefined object object number function function\n"
    );
}

#[test]
fn compiles_function_name_inference_bigints_and_logical_assignment_expressions() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("dynamic.js");
    let output = tempdir.path().join("dynamic.wasm");

    fs::write(
        &input,
        r#"
        let holder = {
          base: 4,
          run: function(value) {
            return arguments[0] + this.base;
          }
        };

        let missing = undefined;
        let named = missing ??= function() {};

        let short = 0;
        let stayed = short &&= unresolved;
        let lifted = Object(2n) + 1n;

        console.log("dynamic", named.name, holder.run(3), stayed, lifted);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "dynamic missing 7 0 3\n"
    );
}

#[test]
fn compiles_anonymous_function_expression_names() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("names.js");
    let output = tempdir.path().join("names.wasm");

    fs::write(
        &input,
        r#"
        let values = [function() {}, function *(value) { yield value + 1; }];
        let plain = values[0];
        let generator = values[1];
        let iterator = generator(3);

        console.log("names", plain.name, generator.name, iterator.next().value, iterator.next().done);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "names   4 true\n");
}

#[test]
fn compiles_generator_arguments_object_across_resumes() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("generator-arguments.js");
    let output = tempdir.path().join("generator-arguments.wasm");

    fs::write(
        &input,
        r#"
        function* g() {
          yield arguments[0];
          yield arguments[1];
          yield arguments[2];
          yield arguments[3];
        }

        let iter = g(23, 45, 33);
        let first = iter.next();
        let second = iter.next();
        let third = iter.next();
        let fourth = iter.next();
        let final = iter.next();

        console.log(
          "generator-arguments",
          first.value,
          second.value,
          third.value,
          typeof fourth.value,
          fourth.done,
          typeof final.value,
          final.done
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "generator-arguments 23 45 33 undefined false undefined true\n"
    );
}

#[test]
fn compiles_generator_with_bindings_across_yields() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("generator-with.js");
    let output = tempdir.path().join("generator-with.wasm");

    fs::write(
        &input,
        r#"
        function* g() {
          var x = 1;
          yield x;
          with ({ x: 2 }) {
            yield x;
          }
          yield x;
        }

        let iter = g();
        let first = iter.next();
        let second = iter.next();
        let third = iter.next();
        let final = iter.next();

        console.log(
          "generator-with",
          first.value,
          second.value,
          third.value,
          typeof final.value,
          final.done
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "generator-with 1 2 1 undefined true\n"
    );
}

#[test]
fn compiles_for_of_arrays_keys_and_top_level_closures() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("for-of.js");
    let output = tempdir.path().join("for-of.wasm");

    fs::write(
        &input,
        r#"
        let array = [0, 1];
        let count = 0;

        for (var value of array) {
          console.log("array", value);
          array.pop();
          count += 1;
        }

        let keys = [];
        for (var key of [10, 20].keys()) {
          keys[key] = key;
        }

        let closures = [undefined, undefined, undefined];
        for (const x of [1, 2, 3]) {
          closures[x - 1] = function() { return x; };
        }

        console.log("for-of", count, keys[0], keys[1], closures[0](), closures[1](), closures[2]());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "array 0\nfor-of 1 0 1 1 2 3\n"
    );
}

#[test]
fn compiles_strict_arguments_for_of_without_length_growth() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arguments-for-of.js");
    let output = tempdir.path().join("arguments-for-of.wasm");

    fs::write(
        &input,
        r#"
        (function() {
          'use strict';
          let values = [];
          let i = 0;
          for (var value of arguments) {
            values[i] = value;
            i += 1;
            arguments[i] *= 2;
          }
          console.log("arguments-for-of", i, values[0], values[1], values[2], arguments.length, arguments[3]);
        }(1, 2, 3));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "arguments-for-of 3 1 4 6 3 NaN\n"
    );
}

#[test]
fn compiles_delete_and_array_builtin_semantics() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("delete.js");
    let output = tempdir.path().join("delete.wasm");

    fs::write(
        &input,
        r#"
        let values = [1, 2, 3];
        values.x = 10;

        let removedIndex = delete values[1];
        let removedProp = delete values.x;
        let removedLength = delete values.length;
        let removedMath = delete Math.LN2;
        let removedJsonProp = delete JSON.stringify;
        let removedJson = delete JSON;

        console.log(
          "delete",
          removedIndex,
          values[1],
          removedProp,
          values.x,
          removedLength,
          values.length,
          removedMath,
          removedJsonProp,
          removedJson,
          typeof JSON,
          Array.isArray(values)
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "delete true undefined true undefined false 3 false true true undefined true\n"
    );
}

#[test]
fn compiles_bitwise_and_shift_operators() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("bitwise.js");
    let output = tempdir.path().join("bitwise.wasm");

    fs::write(
        &input,
        r#"
        let anded = 5 & 3;
        let ored = 5 | 2;
        let xored = 5 ^ 1;
        let shiftedLeft = 3 << 2;
        let shiftedRight = -8 >> 2;
        let shiftedUnsigned = -1 >>> 30;
        let big = (2n << 3n) | 1n;

        console.log("bits", anded, ored, xored, shiftedLeft, shiftedRight, shiftedUnsigned, big);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "bits 1 7 4 12 -2 3 17\n"
    );
}

#[test]
fn compiles_in_operator_for_objects_and_builtins() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("in.js");
    let output = tempdir.path().join("in.wasm");

    fs::write(
        &input,
        r#"
        let present = "" in { "": 0 };
        let missing = "x" in {};
        let builtin = "MAX_VALUE" in Number;

        console.log("in", present, missing, builtin);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "in true false true\n");
}

#[test]
fn compiles_instanceof_hasinstance_and_prototype_getters() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("instanceof.js");
    let output = tempdir.path().join("instanceof.wasm");

    fs::write(
        &input,
        r#"
        let F = {};
        let callCount = 0;
        let seen = undefined;
        let arg0 = undefined;

        F[Symbol.hasInstance] = function() {
          seen = this;
          arg0 = arguments[0];
          callCount += 1;
        };

        let custom = 0 instanceof F;

        Function.prototype.prototype = true;
        let primitive = 0 instanceof Function.prototype;

        let getterCalled = false;
        Object.defineProperty(Function.prototype, "prototype", {
          get: function() {
            getterCalled = true;
            return Array.prototype;
          }
        });

        let viaGetter = [] instanceof Function.prototype;

        console.log(
          "instanceof",
          custom,
          callCount,
          seen === F,
          arg0,
          primitive,
          viaGetter,
          getterCalled
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "instanceof false 1 true 0 false true true\n"
    );
}

#[test]
fn compiles_large_bigint_literals_beyond_i128() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("huge-bigint.js");
    let output = tempdir.path().join("huge-bigint.wasm");

    fs::write(
        &input,
        r#"
        let value = 99022168773993092867842010762644549533696n;
        console.log("huge", value >> 5n);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "huge 3094442774187284152120062836332642172928\n"
    );
}

#[test]
fn compiles_mapped_arguments_aliasing_during_for_of() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("mapped-arguments-for-of.js");
    let output = tempdir.path().join("mapped-arguments-for-of.wasm");

    fs::write(
        &input,
        r#"
        let expected = [1, 3, 1];
        let seen = [];
        let i = 0;

        (function(a, b, c) {
          for (var value of arguments) {
            a = b;
            b = c;
            c = i;
            seen[i] = value;
            i++;
          }
        }(1, 2, 3));

        console.log("mapped-for-of", i, seen[0], seen[1], seen[2]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "mapped-for-of 3 1 3 1\n"
    );
}

#[test]
fn compiles_generator_mapped_arguments_aliasing() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("generator-mapped-arguments.js");
    let output = tempdir.path().join("generator-mapped-arguments.wasm");

    fs::write(
        &input,
        r#"
        function* g(a, b, c, d) {
          arguments[0] = 32;
          arguments[1] = 54;
          arguments[2] = 333;
          console.log("generator-mapped", a, b, c, d);
        }

        let iter = g(23, 45, 33);
        iter.next();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "generator-mapped 32 54 333 undefined\n"
    );
}

#[test]
fn compiles_strict_generators_with_unmapped_arguments() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("strict-generator-arguments.js");
    let output = tempdir.path().join("strict-generator-arguments.wasm");

    fs::write(
        &input,
        r#"
        "use strict";

        function* g(a, b, c, d) {
          arguments[0] = 32;
          arguments[1] = 54;
          arguments[2] = 333;
          console.log("strict-generator", a, b, c, d);
        }

        let iter = g(23, 45, 33);
        iter.next();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "strict-generator 23 45 33 undefined\n"
    );
}

#[test]
fn compiles_arguments_length_property_descriptor() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arguments-descriptor.js");
    let output = tempdir.path().join("arguments-descriptor.wasm");

    fs::write(
        &input,
        r#"
        (function() {
          let desc = Object.getOwnPropertyDescriptor(arguments, "length");
          console.log(
            "arguments-descriptor",
            desc !== undefined,
            desc.value,
            desc.writable,
            desc.enumerable,
            desc.configurable
          );
        }(1, 2, 3));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "arguments-descriptor true 3 true false true\n"
    );
}

#[test]
fn compiles_arguments_callee_property_descriptors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arguments-callee-descriptor.js");
    let output = tempdir.path().join("arguments-callee-descriptor.wasm");

    fs::write(
        &input,
        r#"
        (function sample() {
          let sloppy = Object.getOwnPropertyDescriptor(arguments, "callee");
          console.log(
            "sloppy-callee",
            sloppy !== undefined,
            sloppy.writable,
            sloppy.enumerable,
            sloppy.configurable,
            sloppy.hasOwnProperty("get"),
            sloppy.hasOwnProperty("set")
          );
        }());

        (function() {
          "use strict";
          let strict = Object.getOwnPropertyDescriptor(arguments, "callee");
          console.log(
            "strict-callee",
            strict !== undefined,
            strict.enumerable,
            strict.configurable,
            strict.hasOwnProperty("value"),
            strict.hasOwnProperty("writable"),
            strict.hasOwnProperty("get"),
            strict.hasOwnProperty("set")
          );
        }());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "sloppy-callee true true false true false false\nstrict-callee true false false false false true true\n"
    );
}

#[test]
fn compiles_mapped_arguments_length_from_actual_argument_count() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arguments-length.js");
    let output = tempdir.path().join("arguments-length.wasm");

    fs::write(
        &input,
        r#"
        (function(a, b, c) {
          console.log("arguments-length", arguments.length, arguments[0], arguments[1], arguments[2]);
        }());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "arguments-length 0 undefined undefined undefined\n"
    );
}

#[test]
fn compiles_dynamic_index_reads_on_returned_arguments_objects() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("returned-arguments-dynamic-index.js");
    let output = tempdir.path().join("returned-arguments-dynamic-index.wasm");

    fs::write(
        &input,
        r#"
        function getArgs() {
          return arguments;
        }

        for (var i = 1; i < 5; i++) {
          console.log("returned-arguments", i, getArgs(1, 2, 3, 4, 5)[i]);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "returned-arguments 1 2\nreturned-arguments 2 3\nreturned-arguments 3 4\nreturned-arguments 4 5\n"
    );
}

#[test]
fn compiles_spread_identifier_arrays_for_user_function_calls() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("spread-identifier-call.js");
    let output = tempdir.path().join("spread-identifier-call.wasm");

    fs::write(
        &input,
        r#"
        var arr = [2, 3];

        function capture() {
          console.log("spread-call", arguments.length, arguments[0], arguments[1], arguments[2], arguments[3]);
        }

        capture(42, ...[1], ...arr,);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "spread-call 4 42 1 2 3\n"
    );
}

#[test]
fn compiles_function_apply_with_static_spread_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("function-apply-spread-array.js");
    let output = tempdir.path().join("function-apply-spread-array.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;

        (function() {
          console.log("apply-spread", arguments.length, arguments[0], arguments[1], arguments[2]);
          callCount += 1;
        }.apply(null, [1, 2, 3, ...[]]));

        console.log("apply-count", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "apply-spread 3 1 2 3\napply-count 1\n"
    );
}

#[test]
fn compiles_function_apply_with_static_iterator_spread_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("function-apply-spread-iterator.js");
    let output = tempdir.path().join("function-apply-spread-iterator.wasm");

    fs::write(
        &input,
        r#"
        var iter = {};
        iter[Symbol.iterator] = function() {
          var nextCount = 3;
          return {
            next: function() {
              nextCount += 1;
              return { done: nextCount === 6, value: nextCount };
            }
          };
        };

        (function() {
          console.log("apply-iter", arguments.length, arguments[0], arguments[1], arguments[2], arguments[3], arguments[4]);
        }.apply(null, [1, 2, 3, ...iter]));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "apply-iter 5 1 2 3 4 5\n"
    );
}

#[test]
fn compiles_function_apply_with_static_object_spread_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("function-apply-spread-object.js");
    let output = tempdir.path().join("function-apply-spread-object.wasm");

    fs::write(
        &input,
        r#"
        let source = { c: 3, d: 4 };

        (function(fromIdent, fromNull, fromUndefined) {
          console.log(
            "apply-obj",
            Object.keys(fromIdent).length,
            fromIdent.a,
            fromIdent.b,
            fromIdent.c,
            fromIdent.d,
            Object.keys(fromNull).length,
            fromNull.a,
            fromNull.b,
            Object.keys(fromUndefined).length,
            fromUndefined.a,
            fromUndefined.b
          );
        }.apply(null, [
          { a: 1, b: 2, ...source },
          { a: 5, b: 6, ...null },
          { a: 7, b: 8, ...undefined }
        ]));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "apply-obj 4 1 2 3 4 2 5 6 2 7 8\n"
    );
}

#[test]
fn compiles_function_apply_with_stateful_object_spread_getters() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("function-apply-spread-object-getters.js");
    let output = tempdir
        .path()
        .join("function-apply-spread-object-getters.wasm");

    fs::write(
        &input,
        r#"
        var o = { a: 0, b: 1 };
        var cthulhu = {
          get x() {
            delete o.a;
            o.b = 42;
            o.c = "ni";
          }
        };

        let getterCallCount = 0;
        let repeated = {
          get a() {
            return ++getterCallCount;
          }
        };

        (function(first, second) {
          console.log(
            "apply-obj-getter",
            first.hasOwnProperty("a"),
            first.b,
            first.c,
            first.hasOwnProperty("x"),
            Object.keys(first).length,
            second.a,
            second.c,
            second.d,
            Object.keys(second).length
          );
        }.apply(null, [
          { ...cthulhu, ...o },
          { ...repeated, c: 4, d: 5, a: 42, ...repeated }
        ]));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "apply-obj-getter false 42 ni true 3 2 4 5 3\n"
    );
}

#[test]
fn compiles_private_methods_with_arguments_objects() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("private-method-arguments.js");
    let output = tempdir.path().join("private-method-arguments.wasm");

    fs::write(
        &input,
        r#"
        var observed = false;

        class C {
          #method() {
            observed = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39";
          }

          call() {
            this.#method(42, "TC39",);
          }
        }

        new C().call();
        console.log("private-method", observed);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );
}

#[test]
fn compiles_prototype_generator_method_calls_after_class_lowering() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("prototype-generator-call.js");
    let output = tempdir.path().join("prototype-generator-call.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;

        class C {
          *method() {
            callCount = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39" ? 1 : -1;
          }
        }

        C.prototype.method(42, "TC39",).next();
        console.log("prototype-generator-call", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "prototype-generator-call 1\n"
    );
}

#[test]
fn compiles_object_literal_generator_method_calls() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("object-generator-call.js");
    let output = tempdir.path().join("object-generator-call.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;
        var obj = {
          *method() {
            callCount = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39" ? 1 : -1;
          }
        };

        obj.method(42, "TC39",).next();
        console.log("object-generator-call", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "object-generator-call 1\n"
    );
}

#[test]
fn compiles_class_expression_generator_method_calls_after_lowering() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-expr-generator-call.js");
    let output = tempdir.path().join("class-expr-generator-call.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;

        var C = class {
          *method() {
            callCount = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39" ? 1 : -1;
          }
        };

        C.prototype.method(42, "TC39",).next();
        console.log("class-expr-generator-call", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "class-expr-generator-call 1\n"
    );
}

#[test]
fn compiles_class_generator_method_empty_next_result_properties() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-generator-empty-next.js");
    let output = tempdir.path().join("class-generator-empty-next.wasm");

    fs::write(
        &input,
        r#"
        var result;
        class A {
          *foo(a) {}
        }

        result = A.prototype.foo(3).next();
        console.log("gen-next", result.value, result.done);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "gen-next undefined true\n"
    );
}

#[test]
fn compiles_class_generator_method_conditional_yield_with_implicit_sent_undefined() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-generator-conditional-yield.js");
    let output = tempdir
        .path()
        .join("class-generator-conditional-yield.wasm");

    fs::write(
        &input,
        r#"
        class A {
          *g() { (yield 1) ? yield 2 : yield 3; }
        }

        var iter = A.prototype.g();
        var first = iter.next();
        var second = iter.next();
        var third = iter.next();
        console.log("gen-cond", first.value, first.done, second.value, second.done, third.value, third.done);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "gen-cond 1 false 3 false undefined true\n"
    );
}

#[test]
fn compiles_simple_generator_creation_without_eager_body_execution() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("generator-deferred.js");
    let output = tempdir.path().join("generator-deferred.wasm");

    fs::write(
        &input,
        r#"
        var initCount = 0;
        var iterCount = 0;
        var iter = function*() {
          iterCount += 1;
        }();
        var beforeIter = iterCount;

        var f = ([[] = function() { initCount += 1; return iter; }()]) => {
          console.log("generator-deferred", beforeIter, initCount, iterCount);
        };

        f([]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "generator-deferred 0 1 0\n"
    );
}

#[test]
fn compiles_private_generator_method_alias_getters() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("private-generator-alias.js");
    let output = tempdir.path().join("private-generator-alias.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;

        class C {
          * #method() {
            callCount = arguments.length === 2 && arguments[0] === 42 && arguments[1] === "TC39" ? 1 : -1;
          }

          get method() {
            return this.#method;
          }
        }

        new C().method(42, "TC39",).next();
        console.log("private-generator-alias", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "private-generator-alias 1\n"
    );
}

#[test]
fn compiles_rest_parameters_with_unmapped_arguments() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("rest-arguments.js");
    let output = tempdir.path().join("rest-arguments.wasm");

    fs::write(
        &input,
        r#"
        function rest(a, ...b) {
          arguments[0] = 2;
          console.log("rest-arguments", a, arguments[0], b.length, b[0], b[1]);
        }

        rest(1, 3, 4);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "rest-arguments 1 2 2 3 4\n"
    );
}

#[test]
fn compiles_destructured_parameters_with_unmapped_arguments() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("destructured-arguments.js");
    let output = tempdir.path().join("destructured-arguments.wasm");

    fs::write(
        &input,
        r#"
        let captured = 0;

        function dstr(a, [b]) {
          arguments[0] = 2;
          captured = a + b;
        }

        dstr(1, [4]);
        console.log("destructured-arguments", captured);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "destructured-arguments 5\n"
    );
}

#[test]
fn compiles_function_length_for_destructuring_parameters() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("destructuring-function-length.js");
    let output = tempdir.path().join("destructuring-function-length.wasm");

    fs::write(
        &input,
        r#"
        console.log(
          "destructuring-lengths",
          (([a, b]) => {}).length,
          (function([a, b]) {}).length,
          (function * ([a, b]) {}).length,
          (async ([a, b]) => {}).length,
          (async function([a, b]) {}).length,
          (async function * ([a, b]) {}).length
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "destructuring-lengths 1 1 1 1 1 1\n"
    );
}

#[test]
fn compiles_typed_array_constructor_metadata_reads() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("typed-array-metadata.js");
    let output = tempdir.path().join("typed-array-metadata.wasm");

    fs::write(
        &input,
        r#"
        let ctor = Uint16Array;
        console.log(
          "typed-array-metadata",
          typeof ArrayBuffer,
          typeof Uint8Array,
          Uint8Array.BYTES_PER_ELEMENT,
          ctor.BYTES_PER_ELEMENT,
          Float64Array.BYTES_PER_ELEMENT
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "typed-array-metadata function function 1 2 8\n"
    );
}

#[test]
fn compiles_resizable_typed_array_destructuring_with_arrow_assert_throws() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("resizable-typed-array-destructuring.js");
    let output = tempdir
        .path()
        .join("resizable-typed-array-destructuring.wasm");

    fs::write(
        &input,
        r#"
        function CreateResizableArrayBuffer(byteLength, maxByteLength) {
          return new ArrayBuffer(byteLength, { maxByteLength: maxByteLength });
        }

        var assert = {};
        function sameValue(left, right) {
          if (left === right) {
            return left !== 0 || 1 / left === 1 / right;
          }
          return left !== left && right !== right;
        }

        assert.compareArray = function(actual, expected) {
          if (actual.length !== expected.length) {
            throw new Error("compareArray length");
          }
          for (let i = 0; i < actual.length; ++i) {
            if (!sameValue(actual[i], expected[i])) {
              throw new Error("compareArray value");
            }
          }
        };

        function Convert(item) {
          if (typeof item == "bigint") {
            return Number(item);
          }
          return item;
        }

        function ToNumbers(array) {
          let result = [];
          for (let i = 0; i < array.length; i++) {
            result.push(Convert(array[i]));
          }
          return result;
        }

        function MayNeedBigInt(ta, n) {
          return n;
        }

        let ctors = [Uint8Array];

        for (let ctor of ctors) {
          let rab = CreateResizableArrayBuffer(
            4 * ctor.BYTES_PER_ELEMENT,
            8 * ctor.BYTES_PER_ELEMENT
          );
          let fixedLength = new ctor(rab, 0, 4);
          let lengthTracking = new ctor(rab, 0);
          let taWrite = new ctor(rab);

          for (let i = 0; i < 4; ++i) {
            taWrite[i] = MayNeedBigInt(taWrite, i);
          }

          {
            let [a, b, c, d, e] = fixedLength;
            assert.compareArray(ToNumbers([a, b, c, d]), [0, 1, 2, 3]);
            if (e !== undefined) throw new Error("fixed init");
          }

          rab.resize(3 * ctor.BYTES_PER_ELEMENT);
          __ayyAssertThrows(TypeError, () => {
            let [a, b, c] = fixedLength;
          }, "fixed-oob");

          {
            let [a, b, c, d] = lengthTracking;
            assert.compareArray(ToNumbers([a, b, c]), [0, 1, 2]);
            if (d !== undefined) throw new Error("tracking shrink");
          }

          rab.resize(6 * ctor.BYTES_PER_ELEMENT);
          {
            let [a, b, c, d, e] = fixedLength;
            assert.compareArray(ToNumbers([a, b, c, d]), [0, 0, 0, 0]);
            if (e !== undefined) throw new Error("fixed grow");
          }
        }

        console.log("typedarray-destructuring");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "typedarray-destructuring\n"
    );
}

#[test]
fn compiles_empty_object_destructuring_parameters_throw_type_error_for_nullish() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("destructured-empty-object-nullish.js");
    let output = tempdir
        .path()
        .join("destructured-empty-object-nullish.wasm");

    fs::write(
        &input,
        r#"
        function fn({}) {}

        __ayyAssertThrows(TypeError, function() {
          fn(null);
        }, "null");

        __ayyAssertThrows(TypeError, function() {
          fn(undefined);
        }, "undefined");

        console.log("destructured-empty");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "destructured-empty\n");
}

#[test]
fn compiles_with_proxy_binding_lookup_order_for_keyed_destructuring_defaults() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("with-proxy-destructuring-order.js");
    let output = tempdir.path().join("with-proxy-destructuring-order.wasm");

    fs::write(
        &input,
        r#"
        var log = [];

        var sourceKey = {
          toString: function() {
            log.push("sourceKey");
            return "p";
          }
        };

        var source = {
          get p() {
            log.push("get source");
            return undefined;
          }
        };

        var env = new Proxy({}, {
          has: function(t, pk) {
            log.push("binding::" + pk);
            return false;
          }
        });

        var defaultValue = 0;
        var varTarget;

        with (env) {
          var {
            [sourceKey]: varTarget = defaultValue
          } = source;
        }

        console.log(
          "with-proxy-order",
          log[0],
          log[1],
          log[2],
          log[3],
          log[4],
          log[5],
          varTarget
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "with-proxy-order binding::source binding::sourceKey sourceKey binding::varTarget get source binding::defaultValue 0\n"
    );
}

#[test]
fn compiles_nonconfigurable_mapped_arguments_delete_define_property() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("mapped-arguments-delete.js");
    let output = tempdir.path().join("mapped-arguments-delete.wasm");

    fs::write(
        &input,
        r#"
        (function(a) {
          Object.defineProperty(arguments, "0", { configurable: false });
          let deleted = delete arguments[0];
          Object.defineProperty(arguments, "0", { value: 2 });
          console.log("mapped-arguments-delete", deleted, a, arguments[0]);
        }(1));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "mapped-arguments-delete false 2 2\n"
    );
}

#[test]
fn compiles_mapped_arguments_accessor_define_property() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("mapped-arguments-accessor.js");
    let output = tempdir.path().join("mapped-arguments-accessor.wasm");

    fs::write(
        &input,
        r#"
        (function(a) {
          let setCalls = 0;
          Object.defineProperty(arguments, "0", {
            set(_v) { setCalls += 1; },
            enumerable: true,
            configurable: true,
          });

          arguments[0] = "foo";

          Object.defineProperty(arguments, "1", {
            get: () => "bar",
            enumerable: true,
            configurable: true,
          });

          if (
            setCalls !== 1 ||
            a !== 0 ||
            arguments[0] !== undefined ||
            arguments[1] !== "bar"
          ) {
            throw new Error("mapped accessor mismatch");
          }

          console.log("mapped-arguments-accessor");
        }(0));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "mapped-arguments-accessor\n"
    );
}

#[test]
fn compiles_mapped_arguments_strict_delete_alias() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("mapped-arguments-strict-delete.js");
    let output = tempdir.path().join("mapped-arguments-strict-delete.wasm");

    fs::write(
        &input,
        r#"
        (function(a) {
          Object.defineProperty(arguments, "0", { configurable: false });
          var args = arguments;
          var threw = false;

          try {
            (function() {
              "use strict";
              delete args[0];
            }());
          } catch (_error) {
            threw = true;
          }

          console.log("mapped-arguments-strict-delete", threw, a, args[0]);
        }(1));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "mapped-arguments-strict-delete true 1 1\n"
    );
}

#[test]
fn compiles_mapped_arguments_arrow_strict_delete() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("mapped-arguments-arrow-strict-delete.js");
    let output = tempdir
        .path()
        .join("mapped-arguments-arrow-strict-delete.wasm");

    fs::write(
        &input,
        r#"
        (function(a) {
          Object.defineProperty(arguments, "1", {
            get: () => 3,
            configurable: false,
          });

          let threw = false;
          try {
            (() => {
              "use strict";
              delete arguments[1];
            })();
          } catch (_error) {
            threw = true;
          }

          console.log("mapped-arguments-arrow-strict-delete", threw, arguments[1]);
        }(0));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "mapped-arguments-arrow-strict-delete true 3\n"
    );
}

#[test]
fn compiles_for_in_deletion_during_enumeration() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("for-in-delete.js");
    let output = tempdir.path().join("for-in-delete.wasm");

    fs::write(
        &input,
        r#"
        var obj = Object.create(null);
        var accum = "";

        obj.aa = 1;
        obj.ba = 2;
        obj.ca = 3;

        for (var key in obj) {
          for (var inner in obj) {
            if (inner.indexOf("b") === 0) {
              delete obj[inner];
            }
          }
          accum += key + obj[key];
        }

        console.log("for-in-delete", accum);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "for-in-delete aa1ca3\n"
    );
}

#[test]
fn compiles_for_in_var_redeclaration_in_body() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("for-in-var.js");
    let output = tempdir.path().join("for-in-var.wasm");

    fs::write(
        &input,
        r#"
        var iterCount = 0;

        for (var x in { attr: null }) {
          var x;
          console.log("for-in-var", x);
          iterCount += 1;
        }

        console.log("for-in-count", iterCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "for-in-var attr\nfor-in-count 1\n"
    );
}

#[test]
fn compiles_for_in_key_collectors_returning_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("for-in-keys.js");
    let output = tempdir.path().join("for-in-keys.wasm");

    fs::write(
        &input,
        r#"
        function props(x) {
          var array = [];
          for (let p in x) array.push(p);
          return array;
        }

        let empty = props([]);
        let array = props([1, 2]);
        let object = props({ x: 1, y: 2 });

        console.log(
          "for-in-keys",
          empty.length,
          array.length,
          array[0],
          array[1],
          object.length,
          object[0],
          object[1]
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "for-in-keys 0 2 0 1 2 x y\n"
    );
}

#[test]
fn compiles_labeled_continue() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("labeled-continue.js");
    let output = tempdir.path().join("labeled-continue.wasm");

    fs::write(
        &input,
        r#"
        let count = 0;

        outer: for (let x = 0; x < 3; x++) {
          while (true) {
            count++;
            continue outer;
          }
        }

        console.log("labeled-continue", count);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "labeled-continue 3\n");
}

#[test]
fn compiles_do_while_continue_and_loose_equality() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("do-while-loose-equality.js");
    let output = tempdir.path().join("do-while-loose-equality.wasm");

    fs::write(
        &input,
        r#"
        let total = 0;
        let i = 0;

        do {
          i++;
          if (i < 3) {
            continue;
          }
          total += i;
        } while (i < 4);

        console.log("do-while", total, i);
        console.log("loose-equality", 2 == "2", 2 != "3", null == undefined, 1 == true);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "do-while 7 4\nloose-equality true true true true\n"
    );
}

#[test]
fn compiles_asi_prefix_updates_typeof_function_expressions_and_iife_member_reads() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("asi-function-expressions.js");
    let output = tempdir.path().join("asi-function-expressions.wasm");

    fs::write(
        &input,
        r#"
        var incX = 0;
        var incY = 0;
        incX
        ++incY

        var decX = 1;
        var decY = 1;
        decX
        --decY

        var result = function f(o) { o.x = 1; return o; };
        (new Object()).x;

        var invoked = function f(o) { o.x = 1; return o; }
        (new Object()).x;

        var objectValue =
          1 + (function (t) {
            return { a: t };
          })
          (2 + 3).a;

        var nestedValue =
          1 + (function f(t) {
            return {
              a: function() {
                return t + 1;
              }
            };
          })
          (2 + 3).a();

        console.log(
          "asi",
          incX,
          incY,
          decX,
          decY,
          typeof result,
          invoked,
          objectValue,
          nestedValue
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "asi 0 1 1 0 function 1 6 7\n"
    );
}

#[test]
fn compiles_named_generator_expression_self_binding() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("named-generator-self-binding.js");
    let output = tempdir.path().join("named-generator-self-binding.wasm");

    fs::write(
        &input,
        r#"
        let probeParams;
        let probeBody;

        let fnExpr = function* g(
          _ = (probeParams = function() { return g; })
        ) {
          probeBody = function() { return g; };
        };

        fnExpr().next();

        console.log(
          "generator-self-binding",
          probeParams() === fnExpr,
          probeBody() === fnExpr
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "generator-self-binding true true\n"
    );
}

#[test]
fn compiles_with_scope_capture_and_var_initializers() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("with-scope-capture.js");
    let output = tempdir.path().join("with-scope-capture.wasm");

    fs::write(
        &input,
        r#"
        var o = { prop: "before", x: 0 };
        var f;
        var probeBody;

        with (o) {
          f = function() { return prop; };
          var x = 1, _ = probeBody = function() { return x; };
        }

        o.prop = "after";
        var x = 2;

        console.log("with-scope", f(), probeBody(), x, o.x);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "with-scope after 1 2 1\n"
    );
}

#[test]
fn compiles_with_unscopables_update_expression() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("with-unscopables-update.js");
    let output = tempdir.path().join("with-unscopables-update.wasm");

    fs::write(
        &input,
        r#"
        var unscopablesGetterCalled = 0;
        var a, b, flag = true;

        with (a = { x: 7 }) {
          with (b = { x: 4, get [Symbol.unscopables]() {
            unscopablesGetterCalled++;
            return { x: flag = !flag };
          } }) {
            x++;
          }
        }

        console.log("with-update-inc", unscopablesGetterCalled, a.x, b.x);

        unscopablesGetterCalled = 0;
        flag = true;

        with (a = { x: 7 }) {
          with (b = { x: 4, get [Symbol.unscopables]() {
            unscopablesGetterCalled++;
            return { x: flag = !flag };
          } }) {
            x--;
          }
        }

        console.log("with-update-dec", unscopablesGetterCalled, a.x, b.x);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "with-update-inc 1 7 5\nwith-update-dec 1 7 3\n"
    );
}

#[test]
fn compiles_test262_style_with_unscopables_inc_dec() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("with-unscopables-test262.js");
    let output = tempdir.path().join("with-unscopables-test262.wasm");

    fs::write(
        &input,
        r#"
        function __sameValue(left, right) {
          if (left === right) {
            return left !== 0 || 1 / left === 1 / right;
          }
          return left !== left && right !== right;
        }

        function __assertSameValue(actual, expected, message) {
          if (!__sameValue(actual, expected)) {
            __ayyFail(message ?? "sameValue");
          }
        }

        var unscopablesGetterCalled = 0;
        var a, b, flag = true;
        with (a = { x: 7 }) {
          with (b = { x: 4, get [Symbol.unscopables]() {
                              unscopablesGetterCalled++;
                              return { x: flag=!flag };
                            } }) {
            x++;
          }
        }

        __assertSameValue(unscopablesGetterCalled, 1);
        __assertSameValue(a.x, 7);
        __assertSameValue(b.x, 5);

        unscopablesGetterCalled = 0;
        flag = true;
        with (a = { x: 7 }) {
          with (b = { x: 4, get [Symbol.unscopables]() {
                              unscopablesGetterCalled++;
                              return { x: flag=!flag };
                            } }) {
            x--;
          }
        }

        __assertSameValue(unscopablesGetterCalled, 1);
        __assertSameValue(a.x, 7);
        __assertSameValue(b.x, 3);
        console.log("with-test262", "ok");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "with-test262 ok\n");
}

#[test]
fn compiles_block_scoped_let_bindings() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("block-let.js");
    let output = tempdir.path().join("block-let.wasm");

    fs::write(
        &input,
        r#"
        let x;
        let y = 2;

        {
          let y;
          let x = 3;
          console.log("inner", x, y);
        }

        console.log("outer", x, y);

        if (true) {
          let y;
          console.log("if", y);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "inner 3 undefined\nouter undefined 2\nif undefined\n"
    );
}

#[test]
fn compiles_per_iteration_let_closures() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("for-let-closures.js");
    let output = tempdir.path().join("for-let-closures.wasm");

    fs::write(
        &input,
        r#"
        let a = [];
        for (let i = 0; i < 5; ++i) {
          a.push(function () { return i; });
        }

        console.log("loop-let", a[0](), a[1](), a[2](), a[3](), a[4]());

        let b = [];
        for (let i = 0, f = function() { return i; }; i < 5; ++i) {
          b.push(f);
        }

        console.log("loop-init", b[0](), b[1](), b[2](), b[3](), b[4]());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "loop-let 0 1 2 3 4\nloop-init 0 0 0 0 0\n"
    );
}

#[test]
fn compiles_map_iteration_with_late_insertions() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("map-iteration.js");
    let output = tempdir.path().join("map-iteration.wasm");

    fs::write(
        &input,
        r#"
        let map = new Map();
        map.set(1, 11);
        map.set(2, 22);

        let iterator = map[Symbol.iterator]();
        let first = iterator.next();

        map.set(3, 33);

        let second = iterator.next();
        let third = iterator.next();
        let done = iterator.next();

        map.set(4, 44);

        let stillDone = iterator.next();

        console.log(
          "map",
          first.value[0], first.value[1], first.done,
          second.value[0], second.value[1], second.done,
          third.value[0], third.value[1], third.done,
          done.value, done.done,
          stillDone.value, stillDone.done
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "map 1 11 false 2 22 false 3 33 false undefined true undefined true\n"
    );
}

#[test]
fn compiles_aggregate_error_constructor_and_prototypes() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("aggregate-error.js");
    let output = tempdir.path().join("aggregate-error.wasm");

    fs::write(
        &input,
        r#"
        let called = AggregateError([], "");
        let constructed = new AggregateError([]);
        let noMessage = new AggregateError([], undefined);

        console.log(
          "aggregate",
          Object.getPrototypeOf(AggregateError) === Error,
          Object.getPrototypeOf(AggregateError.prototype) === Error.prototype,
          Object.getPrototypeOf(called) === AggregateError.prototype,
          called instanceof AggregateError,
          Object.getPrototypeOf(constructed) === AggregateError.prototype,
          Object.prototype.hasOwnProperty.call(called, "message"),
          called.message,
          Object.prototype.hasOwnProperty.call(noMessage, "message")
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "aggregate true true true true true true  false\n"
    );
}

#[test]
fn compiles_weakref_constructor_and_deref() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("weakref.js");
    let output = tempdir.path().join("weakref.wasm");

    fs::write(
        &input,
        r#"
        let target = { answer: 42 };
        let weak = new WeakRef(target);

        console.log(
          "weakref",
          Object.getPrototypeOf(WeakRef) === Function.prototype,
          Object.getPrototypeOf(WeakRef.prototype) === Object.prototype,
          weak !== target,
          weak instanceof WeakRef,
          Object.getPrototypeOf(weak) === WeakRef.prototype,
          Object.isExtensible(weak),
          weak.deref() === target,
          WeakRef.prototype.deref.call(weak) === target,
          Object.getOwnPropertySymbols(weak).length
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "weakref true true true true true true true true 0\n"
    );
}

#[test]
fn compiles_math_intrinsics_with_js_number_semantics() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("math-intrinsics.js");
    let output = tempdir.path().join("math-intrinsics.wasm");

    fs::write(
        &input,
        r#"
        let atanNegZero = Math.atan(-0);
        let maxNaN = Math.max({});
        let maxSignedZero = 1 / Math.max(-0, 0);
        let minSignedZero = 1 / Math.min(-0, 0);

        console.log(
          "math",
          typeof Math.SQRT2,
          Math.max.length,
          Math.atan(NaN) !== Math.atan(NaN),
          1 / atanNegZero,
          maxNaN !== maxNaN,
          maxSignedZero,
          minSignedZero
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "math number 2 true -Infinity true Infinity -Infinity\n"
    );
}

#[test]
fn compiles_function_scoped_var_from_nested_block() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("block-var-hoisting.js");
    let output = tempdir.path().join("block-var-hoisting.wasm");

    fs::write(
        &input,
        r#"
        function fn() {
          {
            var x = 1;
            var y;
          }
          console.log("vars", x, y);
        }

        fn();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "vars 1 undefined\n");
}

#[test]
fn compiles_computed_numeric_property_names_using_js_number_strings() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("computed-number-keys.js");
    let output = tempdir.path().join("computed-number-keys.wasm");

    fs::write(
        &input,
        r#"
        let object = {
          [1.2]: "A",
          [1e55]: "B",
          [0.000001]: "C",
          [-0]: "D",
          [Infinity]: "E",
          [-Infinity]: "F",
          [NaN]: "G",
        };

        console.log(
          "keys",
          object["1.2"],
          object["1e+55"],
          object["0.000001"],
          object[0],
          object[Infinity],
          object[-Infinity],
          object[NaN]
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "keys A B C D E F G\n");
}

#[test]
fn compiles_strict_directive_after_other_directives() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("directive-prologue.js");
    let output = tempdir.path().join("directive-prologue.wasm");

    fs::write(
        &input,
        r#"
        function foo() {
          "another directive"
          "use strict";
          console.log("strict", this === undefined);
        }

        foo.call(undefined);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "strict true\n");
}

#[test]
fn does_not_treat_escaped_use_strict_as_a_directive() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("escaped-use-strict.js");
    let output = tempdir.path().join("escaped-use-strict.wasm");

    fs::write(
        &input,
        r#"
        function foo() {
          'use\u0020strict';
          console.log("strict", this !== undefined);
        }

        foo.call(undefined);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "strict true\n");
}

#[test]
fn rejects_use_strict_after_other_directives_before_eval_assignment() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("strict-directive-prologue-eval.js");
    let output = tempdir.path().join("strict-directive-prologue-eval.wasm");

    fs::write(
        &input,
        r#"
        "a";
        "use strict";
        "c";
        eval = 42;
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "strict mode forbids assigning to `eval`");
}

#[test]
fn compiles_sloppy_function_unresolvable_assignment_creates_global() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("sloppy-function-global.js");
    let output = tempdir.path().join("sloppy-function-global.wasm");

    fs::write(
        &input,
        r#"
        function fun() {
          test262unresolvable = null;
          console.log("sloppy-fn-global", test262unresolvable === null);
        }

        fun();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "sloppy-fn-global true\n"
    );
}

#[test]
fn compiles_nested_function_declarations_in_strict_functions_with_runtime_reference_errors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("strict-nested-fn-runtime-reference-error.js");
    let output = tempdir
        .path()
        .join("strict-nested-fn-runtime-reference-error.wasm");

    fs::write(
        &input,
        r#"
        function testcase() {
          "use strict";

          function fun() {
            test262unresolvable = null;
          }

          __ayyAssertThrows(ReferenceError, function() {
            fun();
          }, "nested strict function declaration assignment");
        }

        testcase();
        console.log("ok");
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_top_level_lexicals_separately_from_global_properties() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("global-lexicals.js");
    let output = tempdir.path().join("global-lexicals.wasm");

    fs::write(
        &input,
        r#"
        let Array;
        let token = 42;

        function readArray() {
          return Array;
        }

        function readToken() {
          return token;
        }

        let descriptor = Object.getOwnPropertyDescriptor(this, "Array");
        console.log(
          "global-lex",
          Array === undefined,
          typeof this.Array,
          readArray() === undefined,
          readToken(),
          descriptor.configurable,
          descriptor.enumerable,
          descriptor.writable
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "global-lex true function true 42 true false true\n"
    );
}

#[test]
fn rejects_262_eval_script_under_refined_aot_goal() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("eval-script-rest-patterns.js");
    let output = tempdir.path().join("eval-script-rest-patterns.wasm");

    fs::write(
        &input,
        r#"
        $262.evalScript(
          'function f() { return 1; }' +
          'function f() { return 2; }' +
          'function f() { return 3; }'
        );

        function restArray(...values) {
          console.log("rest", values.constructor === Array, Array.isArray(values));
        }

        function restPattern(...[first, ...rest]) {
          console.log("pattern", first, rest.length, rest[0], rest[1]);
        }

        function restObject(...{a: x, b: y}) {}

        console.log("eval", f());
        restArray(1, 2);
        restPattern(10, 20, 30);
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn compiles_primitive_getters_and_string_replace_callbacks() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("primitive-getters-replace.js");
    let output = tempdir.path().join("primitive-getters-replace.wasm");

    fs::write(
        &input,
        r#"
        "use strict";

        Object.defineProperty(Object.prototype, "x", {
          get: function() {
            return typeof this;
          }
        });

        var captured = "unset";

        function replacer() {
          "use strict";
          captured = this;
          return "a";
        }

        console.log("primitive", (5).x);
        console.log("replace", "ab".replace("b", replacer), captured === undefined);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "primitive number\nreplace aa true\n"
    );
}

#[test]
fn compiles_direct_eval_comment_patterns_with_dynamic_char_insertions() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-comment-patterns.js");
    let output = tempdir.path().join("direct-eval-comment-patterns.wasm");

    fs::write(
        &input,
        r#"
        var yy = 0;
        var xx = String.fromCharCode(0x000B);
        eval("//var " + xx + "yy = -1");
        console.log("single", yy);

        yy = 0;
        xx = String.fromCharCode(0x000A);
        eval("//var " + xx + "yy = -1");
        console.log("line", yy);

        xx = 0;
        eval("/*var " + String.fromCharCode(0x0000) + "xx = 1*/");
        console.log("multi", xx);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "single 0\nline -1\nmulti 0\n"
    );
}

#[test]
fn compiles_indirect_eval_hashbang_script_literals() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("indirect-eval-hashbang.js");
    let output = tempdir.path().join("indirect-eval-hashbang.wasm");

    fs::write(
        &input,
        r#"
        console.log((0, eval)('#!\n') === undefined);
        console.log((0, eval)('#!\n1'));
        console.log((0, eval)('#!2\n') === undefined);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n1\ntrue\n");
}

#[test]
fn compiles_function_constructor_family_alias_hashbang_parse_errors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("function-constructor-family-hashbang.js");
    let output = tempdir
        .path()
        .join("function-constructor-family-hashbang.wasm");

    fs::write(
        &input,
        r#"
        const AsyncFunction = (async function (){}).constructor;
        const GeneratorFunction = (function *(){}).constructor;
        const AsyncGeneratorFunction = (async function *(){}).constructor;
        for (const ctor of [Function, AsyncFunction, GeneratorFunction, AsyncGeneratorFunction]) {
          __ayyAssertThrows(SyntaxError, () => ctor('#!\n_', ''), 'call');
          __ayyAssertThrows(SyntaxError, () => ctor('#!\n_'), 'call-body');
          __ayyAssertThrows(SyntaxError, () => new ctor('#!\n_', ''), 'new-arg');
          __ayyAssertThrows(SyntaxError, () => new ctor('#!\n_'), 'new');
        }
        console.log("function-constructor-family-hashbang");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "function-constructor-family-hashbang\n"
    );
}

#[test]
fn rejects_indirect_eval_and_realm_eval_under_refined_aot_goal() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("indirect-eval.js");
    let output = tempdir.path().join("indirect-eval.wasm");

    fs::write(
        &input,
        r#"
        let x = 1;
        let boxed = { ok: true };

        (function () {
          var local = 0;
          (0, eval)("var local = 2;");
          (0, eval)("function fun() {}");
          console.log("inside", local, typeof fun);
        }());

        let other = $262.createRealm().global;
        let otherEval = other.eval;
        otherEval("var x = 23;");

        console.log(
          "eval",
          (0, eval)(x),
          (0, eval)(boxed) === boxed,
          (0, eval)("this;") === this,
          typeof local,
          typeof fun,
          other.x
        );
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn compiles_cross_realm_indirect_eval_globals() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("cross-realm-indirect-eval.js");
    let output = tempdir.path().join("cross-realm-indirect-eval.wasm");

    fs::write(
        &input,
        r#"
        var other = $262.createRealm().global;
        var otherEval = other.eval;

        otherEval("var x = 23;");
        __assert(typeof x === "undefined");
        __assert(other.x === 23);
        console.log("ok");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn rejects_direct_eval_against_local_function_context() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-local.js");
    let output = tempdir.path().join("direct-eval-local.wasm");

    fs::write(
        &input,
        r#"
        function testcase() {
          var value = "outer";
          function inner() {
            var value = "inner";
            console.log("direct", eval("'inner' === value"));
            console.log("nested", eval("var value = 'eval'; eval(\"value\")"));
          }
          inner();
        }

        testcase();
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn rejects_direct_eval_call_semantics() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-call-semantics.js");
    let output = tempdir.path().join("direct-eval-call-semantics.wasm");

    fs::write(
        &input,
        r#"
        var x = 0;
        try {
          eval(unresolvable);
        } catch (error) {
          console.log("error", error.name);
        }
        console.log("empty", eval() === undefined);
        eval("x = 1", "x = 2");
        console.log("first", x);
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn compiles_direct_eval_assignment_completion_values() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-assignment-completion.js");
    let output = tempdir
        .path()
        .join("direct-eval-assignment-completion.wasm");

    fs::write(
        &input,
        r#"
        var x = {};
        var y;
        var obj = {};

        console.log("assign-object", eval("y = x") === x, y === x);
        console.log("assign-number", eval("y = 7") === 7, y === 7);
        console.log("assign-member", eval("obj.value = x") === x, obj.value === x);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "assign-object true true\nassign-number true true\nassign-member true true\n"
    );
}

#[test]
fn compiles_eval_var_declaration_completion_values() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("eval-var-declaration-completion.wasm.js");
    let output = tempdir.path().join("eval-var-declaration-completion.wasm");

    fs::write(
        &input,
        r#"
        console.log("direct-init", eval("var x = 1") === undefined, x === 1);
        console.log("direct-empty", eval("var y;") === undefined, y === undefined);
        console.log("indirect-init", (0, eval)("var z = 2") === undefined, z === 2);
        console.log("indirect-empty", (0, eval)("var w;") === undefined, w === undefined);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "direct-init true true\ndirect-empty true true\nindirect-init true true\nindirect-empty true true\n"
    );
}

#[test]
fn compiles_module_eval_export_syntax_errors_with_assert_throws() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("module-eval-export-syntax-error.js");
    let output = tempdir.path().join("module-eval-export-syntax-error.wasm");

    fs::write(
        &input,
        r#"
        __ayyAssertThrows(SyntaxError, function() {
          eval("export default null;");
        }, "direct");
        __ayyAssertThrows(SyntaxError, function() {
          (0, eval)("export default null;");
        }, "indirect");
        console.log("ok");
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_module_assert_throws_for_reference_errors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("module-assert-throws-reference-error.js");
    let output = tempdir
        .path()
        .join("module-assert-throws-reference-error.wasm");

    fs::write(
        &input,
        r#"
        __ayyAssertThrows(ReferenceError, function() {
          missing;
        }, "ref");
        console.log("ok");
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_aliased_indirect_eval_against_global_bindings() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("aliased-indirect-eval.js");
    let output = tempdir.path().join("aliased-indirect-eval.wasm");

    fs::write(
        &input,
        r#"
        var x = "str";
        var _eval = eval;
        __assert(_eval("'str' === x"));
        __assert((0, eval)("'str' === x"));
        console.log("ok");
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_nested_aliased_indirect_eval_against_global_bindings() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("nested-aliased-indirect-eval.js");
    let output = tempdir.path().join("nested-aliased-indirect-eval.wasm");

    fs::write(
        &input,
        r#"
        var x = "str";
        function testcase() {
          var _eval = eval;
          function foo() {
            __assert(_eval("'str' === x"));
          }
          foo();
        }
        testcase();
        console.log("ok");
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_direct_and_indirect_eval_class_lexical_isolation() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("eval-class-lexical-isolation.js");
    let output = tempdir.path().join("eval-class-lexical-isolation.wasm");

    fs::write(
        &input,
        r#"
        class outside {}

        eval("class outside {}");
        eval("\"use strict\"; class outside {}");
        eval("class directInner {}");
        __assert(typeof directInner === "undefined");
        __ayyAssertThrows(ReferenceError, function() { directInner; }, "direct");

        (0, eval)("class outside {}");
        (0, eval)("\"use strict\"; class outside {}");
        (0, eval)("class indirectInner {}");
        __assert(typeof indirectInner === "undefined");
        __ayyAssertThrows(ReferenceError, function() { indirectInner; }, "indirect");

        console.log("ok");
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_direct_eval_class_declaration_completion_values() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("eval-class-completion.js");
    let output = tempdir.path().join("eval-class-completion.wasm");

    fs::write(
        &input,
        r#"
        console.log(eval("class C {}") === undefined);
        console.log(eval("1; class C {}") === 1);
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\ntrue\n");
}

#[test]
fn compiles_indirect_eval_against_global_lexical_outside_top_level_block_scopes() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("indirect-eval-global-lexical-heritage.js");
    let output = tempdir
        .path()
        .join("indirect-eval-global-lexical-heritage.wasm");

    fs::write(
        &input,
        r#"
        var actualDirect;
        var actualIndirect;
        var actualIndirectStrict;

        let x = "outside";
        {
          let x = "inside";
          actualDirect = eval("x;");
          actualIndirect = (0, eval)("x;");
          actualIndirectStrict = (0, eval)("\"use strict\"; x;");
        }

        __assert(actualDirect === "inside");
        __assert(actualIndirect === "outside");
        __assert(actualIndirectStrict === "outside");
        console.log("ok");
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_direct_eval_nested_local_and_with_scope_resolution() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-scope-resolution.js");
    let output = tempdir.path().join("direct-eval-scope-resolution.wasm");

    fs::write(
        &input,
        r#"
        var outer = "global";

        function nestedCase() {
          var outer = "middle";
          function inner() {
            var outer = "inner";
            console.log("nested", !!eval("'inner' === outer"), eval("outer") === outer);
          }
          inner();
        }

        function withCase() {
          var env = new Object();
          env.outer = "with";
          var outer = "local";
          with (env) {
            console.log("with", outer === "with", !!eval("'with' === outer"), eval("outer") === outer);
          }
        }

        nestedCase();
        withCase();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "nested true true\nwith true true true\n"
    );
}

#[test]
fn rejects_direct_eval_var_updates_current_function_binding() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-var-binding.js");
    let output = tempdir.path().join("direct-eval-var-binding.wasm");

    fs::write(
        &input,
        r#"
        function f() {
          var value = "outer";
          console.log(eval("var value = 'inner'; 'inner' === value"), value);
        }

        f();
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn rejects_strict_direct_eval_inherited_semantics() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("strict-direct-eval.js");
    let output = tempdir.path().join("strict-direct-eval.wasm");

    fs::write(
        &input,
        r#"
        "use strict";
        function capture(label, source) {
          try {
            eval(source);
            console.log(label, "ok");
          } catch (error) {
            console.log(label, error.name);
          }
        }

        capture("var", "var static;");
        capture("with", "with ({}) {}");
        capture("assign", "unresolvable = null;");
        capture("read", "unresolvable");
        console.log("typeof", eval("typeof unresolvable"));
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn rejects_direct_eval_global_lexical_var_collision_rules() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-global-lex-collision.js");
    let output = tempdir.path().join("direct-eval-global-lex-collision.wasm");

    fs::write(
        &input,
        r#"
        let x;
        try {
          eval("var x;");
          console.log("sloppy", "ok");
        } catch (error) {
          console.log("sloppy", error instanceof SyntaxError);
        }

        eval('"use strict"; var x;');
        console.log("strict-source", "ok");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "sloppy true\nstrict-source ok\n"
    );
}

#[test]
fn rejects_direct_eval_lower_lexical_var_collision_only_for_sloppy_eval() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-lower-lex-collision.js");
    let output = tempdir.path().join("direct-eval-lower-lex-collision.wasm");

    fs::write(
        &input,
        r#"
        {
          let x;
          {
            try {
              eval("var x;");
              console.log("sloppy", "ok");
            } catch (error) {
              console.log("sloppy", error instanceof SyntaxError);
            }
          }
        }

        (function() {
          "use strict";
          {
            let x;
            {
              eval("var x;");
              console.log("strict-caller", "ok");
            }
          }
        })();

        {
          let x;
          {
            eval('"use strict"; var x;');
            console.log("strict-source", "ok");
          }
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "sloppy true\nstrict-caller ok\nstrict-source ok\n"
    );
}

#[test]
fn compiles_direct_eval_arguments_binding_conflicts_in_parameter_scopes() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-arguments-parameter-scope.js");
    let output = tempdir
        .path()
        .join("direct-eval-arguments-parameter-scope.wasm");

    fs::write(
        &input,
        r#"
        const arrowConflict = (p = eval("var arguments = 'param'"), arguments) => {};
        try {
          arrowConflict();
          console.log("arrow-conflict", "ok");
        } catch (error) {
          console.log("arrow-conflict", error instanceof SyntaxError);
        }

        const arrowAllowed = (p = eval("var arguments = 'inner'"), q = () => arguments) => {
          console.log("arrow-allowed", arguments === "inner", q() === "inner");
        };
        arrowAllowed();

        function ordinary(p = eval("var arguments = 'ordinary'")) {}
        try {
          ordinary();
          console.log("ordinary", "ok");
        } catch (error) {
          console.log("ordinary", error instanceof SyntaxError);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "arrow-conflict true\narrow-allowed true true\nordinary true\n"
    );
}

#[test]
fn compiles_arrow_default_parameter_eval_arguments_captures_before_body_shadowing() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("arrow-default-parameter-eval-arguments-capture.js");
    let output = tempdir
        .path()
        .join("arrow-default-parameter-eval-arguments-capture.wasm");

    fs::write(
        &input,
        r#"
        const letCase = (p = eval("var arguments = 'param'"), q = () => arguments) => {
          let arguments = "local";
          console.log("let", q() === "param", q() === "local");
        };
        letCase();

        const varCase = (p = eval("var arguments = 'param'"), q = () => arguments) => {
          var arguments = "local";
          console.log("var", q() === "param", q() === "local");
        };
        varCase();

        const functionCase = (p = eval("var arguments = 'param'"), q = () => arguments) => {
          function arguments() {}
          console.log("function", q() === "param", q() === arguments);
        };
        functionCase();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "let true false\nvar true false\nfunction true false\n"
    );
}

#[test]
fn compiles_arrow_default_parameter_tdz_for_later_and_self_references() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arrow-default-parameter-tdz.js");
    let output = tempdir.path().join("arrow-default-parameter-tdz.wasm");

    fs::write(
        &input,
        r#"
        var laterBodyCount = 0;
        var later = (x = y, y) => {
          laterBodyCount = laterBodyCount + 1;
        };
        try {
          later();
          console.log("later", false);
        } catch (error) {
          console.log("later", error instanceof ReferenceError);
        }
        console.log("later-body", laterBodyCount);

        var prior = (x, y = x) => {
          console.log("prior", y === 1);
        };
        prior(1);

        var selfBodyCount = 0;
        var selfRef = (x = x) => {
          selfBodyCount = selfBodyCount + 1;
        };
        try {
          selfRef();
          console.log("self", false);
        } catch (error) {
          console.log("self", error instanceof ReferenceError);
        }
        console.log("self-body", selfBodyCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "later true\nlater-body 0\nprior true\nself true\nself-body 0\n"
    );
}

#[test]
fn compiles_arrow_destructured_default_unresolvable_references_with_try_catch() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("arrow-destructured-default-unresolvable.js");
    let output = tempdir
        .path()
        .join("arrow-destructured-default-unresolvable.wasm");

    fs::write(
        &input,
        r#"
        var f = ([x = unresolvableReference]) => {};

        try {
          f([]);
          console.log("caught", false);
        } catch (error) {
          console.log("caught", error instanceof ReferenceError);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "caught true\n");
}

#[test]
fn compiles_detached_object_method_aliases_and_async_generator_eval_throws() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("detached-object-method-aliases-and-async-generator-eval.js");
    let output = tempdir
        .path()
        .join("detached-object-method-aliases-and-async-generator-eval.wasm");

    fs::write(
        &input,
        r#"
        let total = 0;
        let o = {
          f(x) {
            total += x;
          },
          async *g(p = eval("var arguments = 'param'"), arguments) {}
        };

        o.f(1);
        let detached = o.f;
        detached(10);
        console.log("plain", total === 11);

        let detachedAsyncGenerator = o.g;
        try {
          detachedAsyncGenerator();
          console.log("async-gen", "ok");
        } catch (error) {
          console.log("async-gen", error instanceof SyntaxError);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "plain true\nasync-gen true\n"
    );
}

#[test]
fn async_default_eval_errors_are_rejected_at_compile_time() {
    let run = Command::new(env!("CARGO_BIN_EXE_test262"))
        .arg("--test262-dir")
        .arg(format!("{}/.cache/test262", env!("CARGO_MANIFEST_DIR")))
        .arg("--contains")
        .arg(
            "test/language/eval-code/direct/async-func-decl-a-following-parameter-is-named-arguments-declare-arguments.js",
        )
        .arg("--limit")
        .arg("1")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(
        run.status.success(),
        "test262 runner failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(
        stdout.contains("COMPILE_FAIL")
            && stdout.contains(
                "test/language/eval-code/direct/async-func-decl-a-following-parameter-is-named-arguments-declare-arguments.js"
            )
            && stdout.contains("runtime source evaluation"),
        "unexpected test262 stdout:\n{stdout}",
    );
    assert!(
        stdout.contains("SUMMARY discovered=1 attempted=1 passed=0 compile_failed=1"),
        "unexpected test262 stdout:\n{stdout}",
    );
}

#[test]
fn rejects_direct_eval_do_while_completion_value() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-do-while.js");
    let output = tempdir.path().join("direct-eval-do-while.wasm");

    fs::write(
        &input,
        r#"
        console.log(eval("do ; while(false)") === undefined);
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn rejects_direct_eval_super_early_errors_before_side_effects() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-super-early-errors.js");
    let output = tempdir.path().join("direct-eval-super-early-errors.wasm");

    fs::write(
        &input,
        r#"
        var executed = false;
        function f() {
          try {
            eval("executed = true; super();");
          } catch (error) {
            console.log("call", error.name, executed);
          }
        }

        var evaluated = false;
        function g() {
          try {
            eval("super[evaluated = true];");
          } catch (error) {
            console.log("prop", error.name, evaluated);
          }
        }

        f();
        g();
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn rejects_direct_eval_new_target_context_rules() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-new-target.js");
    let output = tempdir.path().join("direct-eval-new-target.wasm");

    fs::write(
        &input,
        r#"
        var caught = "none";
        try {
          eval("new.target;");
        } catch (error) {
          caught = error.name;
        }
        console.log("global", caught);

        var arrow = () => {
          try {
            eval("new.target;");
          } catch (error) {
            console.log("arrow", error.name);
          }
        };
        arrow();

        function F() {
          var value = eval("new.target;");
          console.log("function", value === undefined, value === F);
        }

        F();
        new F();
        "#,
    )
    .unwrap();

    assert_cli_compile_rejected(&input, &output, "runtime source evaluation");
}

#[test]
fn compiles_function_this_binding_and_setter_literals() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("function-this.js");
    let output = tempdir.path().join("function-this.wasm");

    fs::write(
        &input,
        r#"
        let global = this;
        let o = {};
        let boxedType;
        let setterThis;

        function strictBound() { "use strict"; return this === o; }
        function sloppyBound() { return this === global; }
        function strictCall() { "use strict"; return typeof this; }
        function sloppyCall() { return typeof this; }

        let obj = { set foo(value) { setterThis = this; } };
        obj.foo = 3;
        boxedType = sloppyCall.call("1");

        console.log(
          "fn",
          strictBound.bind(o)(),
          sloppyBound.bind()(),
          strictCall.call("1"),
          boxedType,
          setterThis === obj
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "fn true true string object true\n"
    );
}

#[test]
fn compiles_async_functions_as_function_instances_returning_promises() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("async-functions.js");
    let output = tempdir.path().join("async-functions.wasm");

    fs::write(
        &input,
        r#"
        async function foo() {
          return 1;
        }

        async function
        bar()
        {
        }

        function baz() {
          return 2;
        }

        let promised = foo();
        let resolved = Promise.resolve(3);

        console.log(
          "async",
          foo instanceof Function,
          bar instanceof Function,
          baz instanceof Function,
          promised instanceof Promise,
          resolved instanceof Promise
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "async true true true true true\n"
    );
}

#[test]
fn compiles_new_for_user_constructors_and_basic_builtins() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("new.js");
    let output = tempdir.path().join("new.wasm");

    fs::write(
        &input,
        r#"
        function Point(x, y) {
          this.x = x;
          this.y = y;
        }

        Point.prototype.sum = function() {
          return this.x + this.y;
        };

        let point = new Point(2, 3);
        let array = new Array(3);
        let object = new Object();
        object.value = point.sum();

        console.log(
          "new",
          point.x,
          point.y,
          point instanceof Point,
          array.length,
          object.value
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "new 2 3 true 3 5\n");
}

#[test]
fn compiles_new_with_constructor_reference_evaluated_before_arguments() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("new-eval-order.js");
    let output = tempdir.path().join("new-eval-order.wasm");

    fs::write(
        &input,
        r#"
        function fn() {
          var x = function() {
            this.foo = 42;
          };

          var result = new x(x = 1);

          console.log("new-order", x, result.foo);
        }

        fn();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "new-order 1 42\n");
}

#[test]
fn compiles_new_object_member_assignment_reads() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("new-object-member-reads.js");
    let output = tempdir.path().join("new-object-member-reads.wasm");

    fs::write(
        &input,
        r#"
        var globalObject = new Object();
        globalObject.left = 1;
        globalObject.right = 1;

        let localObject = new Object();
        localObject.value = 5;

        console.log(
          "new-object-members",
          globalObject.left + globalObject.right,
          globalObject.left,
          globalObject.right,
          localObject.value
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "new-object-members 2 1 1 5\n"
    );
}

#[test]
fn compiles_addition_to_primitive_for_objects_dates_and_functions() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("addition-to-primitive.js");
    let output = tempdir.path().join("addition-to-primitive.wasm");

    fs::write(
        &input,
        r#"
        function f1() { return 0; }
        function f2() { return 0; }
        f2.valueOf = function() { return 1; };
        function f3() { return 0; }
        f3.toString = function() { return 1; };
        function f4() { return 0; }
        f4.valueOf = function() { return -1; };
        f4.toString = function() { return 1; };
        var date = new Date(0);

        var thrownValue = "none";
        try {
          1 + { valueOf: function() { throw "error"; }, toString: function() { return 1; } };
        } catch (error) {
          thrownValue = error;
        }

        var typeErrorCaught = false;
        try {
          1 + { valueOf: function() { return {}; }, toString: function() { return {}; } };
        } catch (error) {
          typeErrorCaught = error instanceof TypeError;
        }

        console.log(
          "addition-primitive",
          { valueOf: function() { return 1; } } + 1 === 2,
          1 + { toString: function() { return 1; } } === 2,
          date.toString() === "Date(0)",
          date + 0 === date.toString() + "0",
          f1.toString() === "function f1() {}",
          f1 + 1 === f1.toString() + 1,
          1 + f2 === 2,
          1 + f3 === 2,
          f4 + 1 === 0,
          thrownValue === "error",
          typeErrorCaught === true
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "addition-primitive true true true true true true true true true true true\n"
    );
}

#[test]
fn compiles_addition_with_boxed_boolean_and_number_wrappers() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("addition-boxed-primitives.js");
    let output = tempdir.path().join("addition-boxed-primitives.wasm");

    fs::write(
        &input,
        r#"
        console.log(
          "addition-boxed",
          new Boolean(true) + true === 2,
          true + new Boolean(true) === 2,
          new Boolean(true) + null === 1,
          null + new Boolean(true) === 1,
          new Number(1) + 1 === 2,
          1 + new Number(1) === 2,
          new String("1") + "1" === "11",
          "1" + new String("1") === "11",
          new String("1") + new String("1") === "11",
          isNaN(new Number(1) + undefined),
          isNaN(undefined + new Number(1))
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "addition-boxed true true true true true true true true true true true\n"
    );
}

#[test]
fn compiles_boolean_builtin_calls_to_primitive_values() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("boolean-builtins.js");
    let output = tempdir.path().join("boolean-builtins.wasm");

    fs::write(
        &input,
        r#"
        console.log(
          "boolean-call",
          Boolean(true) === true,
          Boolean(false) === false,
          Boolean() === false,
          Boolean("") === false,
          Boolean("x") === true
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "boolean-call true true true true true\n"
    );
}

#[test]
fn compiles_addition_with_anonymous_function_expression_to_string() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("addition-anon-fnexpr.js");
    let output = tempdir.path().join("addition-anon-fnexpr.wasm");

    fs::write(
        &input,
        r#"
        console.log(
          "addition-anon-fnexpr",
          ({} + function(){return 1}) === ({}.toString() + function(){return 1}.toString()),
          (function(){return 1} + {}) === (function(){return 1}.toString() + {}.toString()),
          (function(){return 1} + function(){return 1}) === (function(){return 1}.toString() + function(){return 1}.toString())
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "addition-anon-fnexpr true true true\n"
    );
}

#[test]
fn compiles_simple_regexp_exec_no_match_to_null() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("regexp-exec-null.js");
    let output = tempdir.path().join("regexp-exec-null.wasm");

    fs::write(
        &input,
        r#"
        console.log(
          "regexp-exec-null",
          RegExp("0").exec("1") === null
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "regexp-exec-null true\n"
    );
}

#[test]
fn compiles_addition_with_nan_and_infinity_constants() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("addition-nan-infinity.js");
    let output = tempdir.path().join("addition-nan-infinity.wasm");

    fs::write(
        &input,
        r#"
        console.log(
          "addition-nan-infinity",
          isNaN(Number.NaN + 1),
          isNaN(1 + Number.NaN),
          isNaN(Number.NaN + Number.POSITIVE_INFINITY),
          isNaN(Number.POSITIVE_INFINITY + Number.NaN),
          isNaN(Number.NaN + Number.NEGATIVE_INFINITY),
          isNaN(Number.NEGATIVE_INFINITY + Number.NaN)
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "addition-nan-infinity true true true true true true\n"
    );
}

#[test]
fn compiles_addition_with_bigint_wrappers_and_string_coercion() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("addition-bigint.js");
    let output = tempdir.path().join("addition-bigint.wasm");

    fs::write(
        &input,
        r#"
        console.log(
          "addition-bigint",
          0xFEDCBA9876543210n + 0x1n === 0xFEDCBA9876543211n,
          Object(2n) + 1n === 3n,
          1n + Object(2n) === 3n,
          ({ [Symbol.toPrimitive]: function() { return 2n; } }) + 1n === 3n,
          1n + ({ [Symbol.toPrimitive]: function() { return 2n; } }) === 3n,
          ({ valueOf: function() { return 2n; } }) + 1n === 3n,
          1n + ({ toString: function() { return 2n; } }) === 3n,
          1n + "" === "1",
          "" + -1n === "-1"
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "addition-bigint true true true true true true true true true\n"
    );
}

#[test]
fn compiles_addition_symbol_to_primitive_getter_errors_without_unreachable_side_effects() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("addition-symbol-to-primitive-getter-error.js");
    let output = tempdir
        .path()
        .join("addition-symbol-to-primitive-getter-error.wasm");

    fs::write(
        &input,
        r#"
        var thrower = {};
        var counter = {};
        var callCount = 0;
        Object.defineProperty(thrower, Symbol.toPrimitive, {
          get: function() { throw new Error(); }
        });
        Object.defineProperty(counter, Symbol.toPrimitive, {
          get: function() { callCount = callCount + 1; }
        });

        try { thrower + counter; } catch (e) {}
        try { counter + thrower; } catch (e) {}

        console.log("addition-symbol-getter-order", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "addition-symbol-getter-order 1\n"
    );
}

#[test]
fn compiles_addition_order_of_evaluation_for_valueof_throws() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("addition-order-of-evaluation.js");
    let output = tempdir.path().join("addition-order-of-evaluation.wasm");

    fs::write(
        &input,
        r#"
        function MyError() {}
        var trace = "";

        try {
          (function() {
            trace += "1";
            return { valueOf: function() { trace += "3"; throw new MyError(); } };
          })() + (function() {
            trace += "2";
            return { valueOf: function() { trace += "4"; throw new Error("should not run"); } };
          })();
        } catch (e) {
          console.log("addition-order-case1", trace, !!e);
        }

        trace = "";
        try {
          (function() {
            trace += "1";
            return { valueOf: function() { trace += "3"; return 1; } };
          })() + (function() {
            trace += "2";
            return { valueOf: function() { trace += "4"; throw new MyError(); } };
          })();
        } catch (e) {
          console.log("addition-order-case2", trace, !!e);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "addition-order-case1 123 true\naddition-order-case2 1234 true\n"
    );
}

#[test]
fn compiles_static_if_folding_without_evaluating_pure_divide_by_zero_conditions() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("static-if-divzero.js");
    let output = tempdir.path().join("static-if-divzero.wasm");

    fs::write(
        &input,
        r#"
        if (1 / (-0 + -0) !== Number.NEGATIVE_INFINITY) {
          console.log("bad-neg-zero");
        }

        if (1 / (-Number.MIN_VALUE + Number.MIN_VALUE) !== Number.POSITIVE_INFINITY) {
          console.log("bad-pos-zero");
        }

        console.log("static-if-divzero", "ok");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "static-if-divzero ok\n"
    );
}

#[test]
fn compiles_new_target_for_calls_and_constructors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("new-target.js");
    let output = tempdir.path().join("new-target.wasm");

    fs::write(
        &input,
        r#"
        function F() {
          console.log(
            "new-target",
            new.target === undefined,
            new.target === F
          );
        }

        F();
        new F();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "new-target true false\nnew-target false true\n"
    );
}

#[test]
fn compiles_direct_eval_new_target_in_function_code() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("direct-eval-new-target-function.js");
    let output = tempdir.path().join("direct-eval-new-target-function.wasm");

    fs::write(
        &input,
        r#"
        var newTarget = null;
        var getNewTarget = function() {
          newTarget = eval("new.target;");
        };

        getNewTarget();
        console.log("direct-eval-call", newTarget === undefined);

        new getNewTarget();
        console.log("direct-eval-new", newTarget === getNewTarget);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "direct-eval-call true\ndirect-eval-new true\n"
    );
}

#[test]
fn compiles_direct_eval_non_definable_global_function_throws() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-non-definable-global-function.js");
    let output = tempdir
        .path()
        .join("direct-eval-non-definable-global-function.wasm");

    fs::write(
        &input,
        r#"
        var threw = false;
        try {
          eval("function NaN(){}");
        } catch (error) {
          threw = true;
        }
        console.log("eval-non-definable-global-function", threw);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "eval-non-definable-global-function true\n"
    );
}

#[test]
fn compiles_direct_eval_global_function_initialization_references() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-global-function-initialization.js");
    let output = tempdir
        .path()
        .join("direct-eval-global-function-initialization.wasm");

    fs::write(
        &input,
        r#"
        var initial;
        eval("initial = f; function f() { return 234; }");
        console.log("eval-global-function-init-type", typeof initial);
        console.log("eval-global-function-init-call", initial());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "eval-global-function-init-type function\neval-global-function-init-call 234\n"
    );
}

#[test]
fn compiles_direct_eval_global_function_initialization_with_preceding_global_object() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-global-function-initialization-overlap.js");
    let output = tempdir
        .path()
        .join("direct-eval-global-function-initialization-overlap.wasm");

    fs::write(
        &input,
        r#"
        var x = {};
        var initial;
        eval("initial = f; function f() { return 234; }");
        console.log("eval-global-function-init-overlap-type", typeof initial);
        console.log("eval-global-function-init-overlap-call", initial());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "eval-global-function-init-overlap-type function\neval-global-function-init-overlap-call 234\n"
    );
}

#[test]
fn compiles_indirect_eval_global_function_initialization_references() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("indirect-eval-global-function-initialization.js");
    let output = tempdir
        .path()
        .join("indirect-eval-global-function-initialization.wasm");

    fs::write(
        &input,
        r#"
        var initial;
        (0, eval)("initial = f; function f() { return 234; }");
        console.log("indirect-eval-global-function-init-type", typeof initial);
        console.log("indirect-eval-global-function-init-call", initial());
        console.log("indirect-eval-global-function-global-type", typeof f);
        console.log("indirect-eval-global-function-global-call", f());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "indirect-eval-global-function-init-type function\nindirect-eval-global-function-init-call 234\nindirect-eval-global-function-global-type function\nindirect-eval-global-function-global-call 234\n"
    );
}

#[test]
fn compiles_strict_indirect_eval_function_declarations_without_global_leakage() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("strict-indirect-eval-function-scope.js");
    let output = tempdir
        .path()
        .join("strict-indirect-eval-function-scope.wasm");

    fs::write(
        &input,
        r#"
        var typeofInside;

        (function() {
          (0, eval)("\"use strict\"; function fun(){}");
          typeofInside = typeof fun;
        }());

        console.log("strict-indirect-eval-fn-inside", typeofInside);
        console.log("strict-indirect-eval-fn-outside", typeof fun);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "strict-indirect-eval-fn-inside undefined\nstrict-indirect-eval-fn-outside undefined\n"
    );
}

#[test]
fn compiles_indirect_eval_global_lexical_var_collisions_throw_syntax_error() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("indirect-eval-global-lexical-collision.js");
    let output = tempdir
        .path()
        .join("indirect-eval-global-lexical-collision.wasm");

    fs::write(
        &input,
        r#"
        let x;
        var caught;

        try {
          (0, eval)("var x;");
        } catch (error) {
          caught = error;
        }

        console.log("indirect-eval-global-lexical-collision", typeof caught, caught.constructor === SyntaxError);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "indirect-eval-global-lexical-collision object true\n"
    );
}

#[test]
fn compiles_indirect_eval_global_var_initialization_and_scope_separation() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("indirect-eval-global-var-initialization.js");
    let output = tempdir
        .path()
        .join("indirect-eval-global-var-initialization.wasm");

    fs::write(
        &input,
        r#"
        var initialExisting;
        var x = 23;
        (0, eval)("initialExisting = x; var x = 45;");
        var existingDesc = Object.getOwnPropertyDescriptor(this, "x");
        console.log(
          "indirect-eval-global-var-existing",
          initialExisting,
          x,
          existingDesc.value,
          existingDesc.writable,
          existingDesc.enumerable,
          existingDesc.configurable
        );

        var initialNew = null;
        (0, eval)("initialNew = y; var y = 9;");
        var newDesc = Object.getOwnPropertyDescriptor(this, "y");
        console.log(
          "indirect-eval-global-var-new",
          initialNew === undefined,
          typeof y,
          y,
          newDesc.value,
          newDesc.writable,
          newDesc.enumerable,
          newDesc.configurable
        );

        (function() {
          var x = 0;
          (0, eval)("var x = 1;");
          console.log("indirect-eval-var-scope-inner", x);
        }());
        console.log("indirect-eval-var-scope-outer", x);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "indirect-eval-global-var-existing 23 45 45 true true false\nindirect-eval-global-var-new true number 9 9 true true true\nindirect-eval-var-scope-inner 0\nindirect-eval-var-scope-outer 1\n"
    );
}

#[test]
fn compiles_direct_eval_global_var_initialization_through_verify_property_harness() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-global-var-initialization.js");
    let output = tempdir
        .path()
        .join("direct-eval-global-var-initialization.wasm");

    fs::write(
        &input,
        r#"
        function verifyProperty() {
          console.log("fallback");
        }

        var initialExisting;
        var x = 23;
        eval("initialExisting = x; var x = 45;");
        verifyProperty(this, "x", {
          value: 45,
          writable: true,
          enumerable: true,
          configurable: false,
        });
        console.log("existing", initialExisting, x, delete x);

        var initialNew = null;
        eval("initialNew = y; var y;");
        verifyProperty(this, "y", {
          value: undefined,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        console.log("new-initial", initialNew === undefined);
        console.log("new-type", typeof y, y === undefined);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "existing 23 45 false\nnew-initial true\nnew-type undefined true\n"
    );
}

#[test]
fn compiles_direct_eval_local_function_initialization_create_update_and_delete() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-local-function-initialization.js");
    let output = tempdir
        .path()
        .join("direct-eval-local-function-initialization.wasm");

    fs::write(
        &input,
        r#"
        var initialNew, postAssignment, outerNewReadThrows;
        (function() {
          eval("initialNew = f; f = 5; postAssignment = f; function f() { return 33; }");
        }());
        try { f; outerNewReadThrows = false; } catch (error) { outerNewReadThrows = error instanceof ReferenceError; }
        console.log("eval-local-new-type", typeof initialNew);
        console.log("eval-local-new-call", initialNew());
        console.log("eval-local-new-post", postAssignment);
        console.log("eval-local-new-outer", outerNewReadThrows);

        var initialUpdate, postUpdate, outerUpdateReadThrows;
        (function() {
          var f = 88;
          eval("initialUpdate = f; function f() { return 44; }");
          postUpdate = f();
        }());
        try { f; outerUpdateReadThrows = false; } catch (error) { outerUpdateReadThrows = error instanceof ReferenceError; }
        console.log("eval-local-update-type", typeof initialUpdate);
        console.log("eval-local-update-call", initialUpdate());
        console.log("eval-local-update-post", postUpdate);
        console.log("eval-local-update-outer", outerUpdateReadThrows);

        var initialDelete, deleteResult, afterDeleteThrows;
        (function() {
          eval("initialDelete = f; deleteResult = delete f; try { f; afterDeleteThrows = false; } catch (error) { afterDeleteThrows = error instanceof ReferenceError; } function f() { return 55; }");
        }());
        console.log("eval-local-delete-type", typeof initialDelete);
        console.log("eval-local-delete-call", initialDelete());
        console.log("eval-local-delete-result", deleteResult);
        console.log("eval-local-delete-after", afterDeleteThrows);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "eval-local-new-type function\n\
eval-local-new-call 33\n\
eval-local-new-post 5\n\
eval-local-new-outer true\n\
eval-local-update-type function\n\
eval-local-update-call 44\n\
eval-local-update-post 44\n\
eval-local-update-outer true\n\
eval-local-delete-type function\n\
eval-local-delete-call 55\n\
eval-local-delete-result true\n\
eval-local-delete-after true\n"
    );
}

#[test]
fn compiles_sloppy_direct_eval_function_declarations_in_caller_var_env() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-sloppy-local-function-scope.js");
    let output = tempdir
        .path()
        .join("direct-eval-sloppy-local-function-scope.wasm");

    fs::write(
        &input,
        r#"
        var typeofInside, callInside;
        (function() {
          eval("function fun() { return 73; }");
          typeofInside = typeof fun;
          callInside = fun();
        }());
        console.log("eval-sloppy-local-function-typeof-inside", typeofInside);
        console.log("eval-sloppy-local-function-call-inside", callInside);
        console.log("eval-sloppy-local-function-typeof-outside", typeof fun);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "eval-sloppy-local-function-typeof-inside function\n\
eval-sloppy-local-function-call-inside 73\n\
eval-sloppy-local-function-typeof-outside undefined\n"
    );
}

#[test]
fn compiles_nested_function_closure_reads_with_strict_direct_eval_var_isolation() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-eval-strict-nested-closure-vars.js");
    let output = tempdir
        .path()
        .join("direct-eval-strict-nested-closure-vars.wasm");

    fs::write(
        &input,
        r#"
        function strictSource() {
          var value = 0;
          function inner() {
            eval("'use strict'; var value = 1;");
            console.log("strict-source", value);
          }
          inner();
        }

        function strictCaller() {
          "use strict";
          var value = 0;
          function inner() {
            eval("var value = 1;");
            console.log("strict-caller", value);
          }
          inner();
        }

        function plainClosure() {
          var value = 2;
          function inner() {
            return value;
          }
          console.log("plain-closure", inner());
        }

        strictSource();
        strictCaller();
        plainClosure();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "strict-source 0\nstrict-caller 0\nplain-closure 2\n"
    );
}

#[test]
fn compiles_new_target_via_call_apply_and_tagged_templates() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("new-target-call-apply.js");
    let output = tempdir.path().join("new-target-call-apply.wasm");

    fs::write(
        &input,
        r#"
        var newTarget = null;

        function f() {
          newTarget = new.target;
        }

        f.call({});
        console.log("new-target-call", newTarget === undefined);

        f.apply({});
        console.log("new-target-apply", newTarget === undefined);

        f``;
        console.log("new-target-tag", newTarget === undefined);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "new-target-call true\nnew-target-apply true\nnew-target-tag true\n"
    );
}

#[test]
fn compiles_mixed_bigint_arithmetic_and_comparisons() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("bigint-mixed.js");
    let output = tempdir.path().join("bigint-mixed.wasm");

    fs::write(
        &input,
        r#"
        let wrapped = Object(3n);
        console.log(
          "bigint",
          wrapped - 1n,
          wrapped * 2n,
          2n == "2",
          0n != "",
          2n <= "3",
          4n <= 3,
          0n <= Number.MIN_VALUE
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "bigint 2 6 true false true false true\n"
    );
}

#[test]
fn compiles_labeled_continue_and_do_while() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("labeled.js");
    let output = tempdir.path().join("labeled.wasm");

    fs::write(
        &input,
        r#"
        let count = 0;
        let outerRuns = 0;

        outer: for (let i = 0; i < 4; i++) {
          outerRuns += 1;
          let j = 0;
          do {
            j += 1;
            if (j === 2) {
              continue outer;
            }
            count += 100;
          } while (j < 3);
        }

        console.log("labels", count, outerRuns);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "labels 400 4\n");
}

#[test]
fn compiles_script_await_labels() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("await-label.js");
    let output = tempdir.path().join("await-label.wasm");

    fs::write(
        &input,
        r#"
        await: console.log("await-label", 1);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "await-label 1\n");
}

#[test]
fn compiles_script_await_identifiers_nested_in_async_functions() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("await-ident-nested.js");
    let output = tempdir.path().join("await-ident-nested.wasm");

    fs::write(
        &input,
        r#"
        var await;

        async function foo() {
          function bar() {
            await = 1;
          }
          bar();
        }

        foo();
        console.log("await-nested", await);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "await-nested 1\n");
}

#[test]
fn compiles_async_functions_named_await_in_scripts() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("await-function-name.js");
    let output = tempdir.path().join("await-function-name.wasm");

    fs::write(
        &input,
        r#"
        async function await() { return 1; }
        console.log("await-name", await instanceof Function);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "await-name true\n");
}

#[test]
fn compiles_switch_fallthrough_and_break() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("switch-flow.js");
    let output = tempdir.path().join("switch-flow.wasm");

    fs::write(
        &input,
        r#"
        let fromOne = "";
        switch (1) {
          case 1:
            fromOne += "a";
          case 2:
            fromOne += "b";
            break;
          default:
            fromOne += "c";
        }

        let fromTwo = "";
        switch (2) {
          case 1:
            fromTwo += "a";
          case 2:
            fromTwo += "b";
            break;
          default:
            fromTwo += "c";
        }

        console.log("switch-flow", fromOne, fromTwo);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "switch-flow ab b\n");
}

#[test]
fn compiles_switch_case_lexical_scope() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("switch-scope.js");
    let output = tempdir.path().join("switch-scope.wasm");

    fs::write(
        &input,
        r#"
        let x = "outside";
        var probeExpr, probeSelector, probeStmt;

        switch (probeExpr = function() { return x; }, null) {
          case probeSelector = function() { return x; }, null:
            probeStmt = function() { return x; };
            let x = "inside";
        }

        console.log("switch-scope", probeExpr(), probeSelector(), probeStmt());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "switch-scope outside inside inside\n"
    );
}

#[test]
fn compiles_switch_with_fallthrough_and_case_scope() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("switch.js");
    let output = tempdir.path().join("switch.wasm");

    fs::write(
        &input,
        r#"
        let probe = "outer";
        let capture = function () {
          return "missing";
        };

        switch ((probe = "expr", 2)) {
          case (probe = "case-1", 1):
            probe = "wrong";
            break;
          case (probe = "case-2", 2):
            let value = "inside";
            capture = function () {
              return value;
            };
          default:
            probe = probe + "-fall";
        }

        console.log("switch", probe, capture());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "switch case-2-fall inside\n"
    );
}

#[test]
fn compiles_unicode_surrogate_pair_regexp_literals() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("regexp-unicode.js");
    let output = tempdir.path().join("regexp-unicode.wasm");

    fs::write(
        &input,
        r#"
        let escaped = /^[\ud834\udf06]$/u.test('\ud834\udf06');
        let literal = /^[𝌆]$/u.test('𝌆');
        console.log("regexp-unicode", escaped, literal);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "regexp-unicode true true\n"
    );
}

#[test]
fn compiles_regexp_unicode_null_escape_and_case_mapping() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("regexp-runtime.js");
    let output = tempdir.path().join("regexp-runtime.wasm");

    fs::write(
        &input,
        r#"
        let nullChar = String.fromCharCode(0);
        let nullExec = /\0/u.exec(nullChar)[0] === nullChar;
        let nullTest = /^\0a$/u.test('\0a');
        let nullMatch = '\x00②'.match(/\0②/u)[0] === '\x00②';
        let nullSearch = '\u0000፬'.search(/\0፬$/u);
        let casePlain = /\u212a/i.test('k');
        let caseUnicode = /\u212a/iu.test('k');
        let unicodeQ = /\u{3f}/u.test('?');
        let unicodeQZero = /\u{000000003f}/u.test('?');
        let unicodeQUpper = /\u{3F}/u.test('?');
        console.log("regexp-runtime", nullExec, nullTest, nullMatch, nullSearch, casePlain, caseUnicode, unicodeQ, unicodeQZero, unicodeQUpper);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "regexp-runtime true true true 0 false true true true true\n"
    );
}

#[test]
fn compiles_named_group_forward_references_in_regexps() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("regexp-named-groups.js");
    let output = tempdir.path().join("regexp-named-groups.wasm");

    fs::write(
        &input,
        r#"
        console.log("regexp-named", /\k<a>(?<a>x)/.test("x"));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "regexp-named true\n");
}

#[test]
fn compiles_module_anonymous_default_export_declaration() {
    let tempdir = tempdir().unwrap();
    let output = tempdir.path().join("module-export-default.wasm");
    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_source_with_goal(
        r#"
        var count = 0;
        export default function* () {} if (true) { count += 1; }
        console.log("module-default", count);
        "#,
        &options,
        true,
    )
    .unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "module-default 1\n");
}

#[test]
fn compiles_module_anonymous_default_export_class_name() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        export default class { valueOf() { return 45; } }
        import C from "./entry.js";
        console.log("module-default-class", new C().valueOf(), C.name);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "module-default-class 45 default\n"
    );
}

#[test]
fn compiles_module_named_default_export_declaration_binding() {
    let tempdir = tempdir().unwrap();
    let output = tempdir.path().join("module-export-default-binding.wasm");
    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_source_with_goal(
        r#"
        export default async function A() {}
        A.foo = "ok";
        console.log("module-default-binding", typeof A, A.foo);
        "#,
        &options,
        true,
    )
    .unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "module-default-binding function ok\n"
    );
}

#[test]
fn compiles_module_named_default_generator_export_declaration() {
    let tempdir = tempdir().unwrap();
    let output = tempdir.path().join("module-export-default-generator.wasm");
    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_source_with_goal(
        r#"
        var count = 0;
        export default function* g() {} if (true) { count += 1; }
        console.log("module-default-generator", count, typeof g);
        "#,
        &options,
        true,
    )
    .unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "module-default-generator 1 function\n"
    );
}

#[test]
fn compiles_module_export_declarations_without_imports() {
    let tempdir = tempdir().unwrap();
    let output = tempdir.path().join("module-export.wasm");
    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_source_with_goal(
        r#"
        export const answer = 42;
        export function read() { return answer; }
        console.log("module-export", read());
        "#,
        &options,
        true,
    )
    .unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "module-export 42\n");
}

#[test]
fn compiles_module_namespace_self_import_initialization() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import * as ns from "./entry.js";
        export let localUninit1 = 111;
        let localUninit2 = 222;
        export { localUninit2 as renamedUninit };
        export { localUninit1 as indirectUninit } from "./entry.js";
        export default 333;

        console.log(ns.localUninit1, ns.renamedUninit, ns.indirectUninit, ns.default);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "111 222 111 333\n");
}

#[test]
fn compiles_module_default_export_expression_function_name_inference() {
    let tempdir = tempdir().unwrap();
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export default (function() { return 99; });
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import f from "./fixture.js";
        console.log(f(), f.name);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "99 default\n");
}

#[test]
fn compiles_module_default_export_expression_class_static_name_method() {
    let tempdir = tempdir().unwrap();
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export default (class { static name() { return "name method"; } });
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import C from "./fixture.js";
        console.log(C.name());
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "name method\n");
}

#[test]
fn compiles_module_default_export_class_static_name_method() {
    let tempdir = tempdir().unwrap();
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export default class { static name() { return "name method"; } }
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import C from "./fixture.js";
        console.log(C.name());
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "name method\n");
}

#[test]
fn compiles_module_default_export_function_declarations_with_instantiation_names() {
    let tempdir = tempdir().unwrap();
    let anon_fixture = tempdir.path().join("anon.js");
    let named_fixture = tempdir.path().join("named.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &anon_fixture,
        r#"
        import f from "./anon.js";
        console.log(f(), f.name);
        export default function() { return 23; }
        "#,
    )
    .unwrap();
    fs::write(
        &named_fixture,
        r#"
        import f from "./named.js";
        console.log(f(), f.name);
        export default function fName() { return 29; }
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import "./anon.js";
        import "./named.js";
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "23 default\n29 fName\n"
    );
}

#[test]
fn compiles_module_default_export_generator_declarations_with_instantiation_names() {
    let tempdir = tempdir().unwrap();
    let anon_fixture = tempdir.path().join("anon-gen.js");
    let named_fixture = tempdir.path().join("named-gen.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &anon_fixture,
        r#"
        import g from "./anon-gen.js";
        console.log(g().next().value, g.name);
        export default function* () { return 23; }
        "#,
    )
    .unwrap();
    fs::write(
        &named_fixture,
        r#"
        import g from "./named-gen.js";
        console.log(g().next().value, g.name);
        export default function* gName() { return 31; }
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import "./anon-gen.js";
        import "./named-gen.js";
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "23 default\n31 gName\n"
    );
}

#[test]
fn module_const_bindings_stay_immutable_inside_nested_functions() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        let before;
        try {
          typeof test262;
          before = "no-throw";
        } catch (error) {
          before = error.name;
        }

        const test262 = 23;

        let assignResult;
        try {
          (function() {
            test262 = null;
          })();
          assignResult = "no-throw";
        } catch (error) {
          assignResult = error.name;
        }

        console.log(
          before,
          assignResult,
          test262,
          Object.getOwnPropertyDescriptor(globalThis, "test262")
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "ReferenceError TypeError 23 undefined\n"
    );
}

#[test]
fn rejects_module_fixtures_that_use_function_constructor() {
    let tempdir = tempdir().unwrap();
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        new Function("return this;")().test262 = 262;
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import "./fixture.js";
        console.log(globalThis.test262);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&entry, &options, true).unwrap_err();
    assert!(error.to_string().contains("runtime source evaluation"));
}

#[test]
fn module_private_fields_allow_inner_class_access_to_outer_private_names() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        class outer {
          #x = 42;

          f() {
            var self = this;
            return class inner {
              g() {
                return self.#x;
              }
            };
          }
        }

        var innerclass = new outer().f();
        console.log(new innerclass().g());
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");
}

#[test]
fn compiles_module_namespace_fixtures_with_class_declarations() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let middle = tempdir.path().join("middle.js");
    let leaf = tempdir.path().join("leaf.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(&middle, r#"export * as exportns from "./leaf.js";"#).unwrap();
    fs::write(
        &leaf,
        r#"
        class notExportedClass {}
        export class starAsClassDecl {}
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import * as ns from "./middle.js";
        console.log("starAsClassDecl" in ns.exportns, "nonExportedClass" in ns.exportns);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true false\n");
}

#[test]
fn module_namespace_descriptors_expose_data_values_for_exports() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import * as ns from "./entry.js";
        export var local1 = 201;
        var local2 = 207;
        export { local2 as renamed };
        export { local1 as indirect } from "./entry.js";
        export default 302;

        let desc = Object.getOwnPropertyDescriptor(ns, "local1");
        let renamed = Object.getOwnPropertyDescriptor(ns, "renamed");
        let indirect = Object.getOwnPropertyDescriptor(ns, "indirect");
        let dflt = Object.getOwnPropertyDescriptor(ns, "default");

        console.log(
          desc.value,
          desc.writable,
          desc.enumerable,
          desc.configurable,
          renamed.value,
          indirect.value,
          dflt.value
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "201 true true false 207 201 302\n"
    );
}

#[test]
fn reflect_has_matches_module_namespace_property_queries() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import * as ns from "./entry.js";
        var test262;
        export { test262 as anotherName };
        export var local1;
        export default null;

        console.log(
          Reflect.has(ns, "local1"),
          Reflect.has(ns, "default"),
          Reflect.has(ns, "test262"),
          Reflect.has(ns, "__proto__")
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "true true false false\n"
    );
}

#[test]
fn module_namespace_has_property_ignores_export_initialization_state() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import * as ns from "./entry.js";

        console.log(
          "local1" in ns,
          Reflect.has(ns, "local1"),
          "renamed" in ns,
          Reflect.has(ns, "renamed"),
          "indirect" in ns,
          Reflect.has(ns, "indirect"),
          "default" in ns,
          Reflect.has(ns, "default")
        );

        export let local1 = 23;
        let local2 = 45;
        export { local2 as renamed };
        export { local1 as indirect } from "./entry.js";
        export default null;
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "true true true true true true true true\n"
    );
}

#[test]
fn module_namespace_own_keys_include_export_names_and_tostringtag() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export var g_star;
        export { g_star as h_starRenamed };
        export { a_local1 as i_starIndirect } from "./entry.js";
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import * as ns from "./entry.js";
        export var a_local1;
        var local2;
        export { local2 as b_renamed };
        export { a_local1 as e_indirect } from "./entry.js";
        export * from "./fixture.js";
        export let c_localUninit1;
        let localUninit2;
        export { localUninit2 as d_renamedUninit };
        export { c_localUninit1 as f_indirectUninit } from "./entry.js";
        export default null;

        let stringKeys = Object.getOwnPropertyNames(ns);
        let symbolKeys = Object.getOwnPropertySymbols(ns);
        let allKeys = Reflect.ownKeys(ns);
        console.log(
          stringKeys.length,
          stringKeys[0],
          stringKeys[9],
          symbolKeys.length,
          symbolKeys.indexOf(Symbol.toStringTag),
          allKeys.length,
          allKeys[0],
          allKeys[9],
          allKeys.indexOf(Symbol.toStringTag)
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "10 a_local1 i_starIndirect 1 0 11 a_local1 i_starIndirect 10\n"
    );
}

#[test]
fn module_namespace_omits_ambiguous_star_exports() {
    let tempdir = tempdir().unwrap();
    let first = tempdir.path().join("first.js");
    let second = tempdir.path().join("second.js");
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &first,
        r#"
        export var first = null;
        export var both = null;
        "#,
    )
    .unwrap();
    fs::write(
        &second,
        r#"
        export var second = null;
        export var both = null;
        "#,
    )
    .unwrap();
    fs::write(
        &fixture,
        r#"
        export * from "./first.js";
        export * from "./second.js";
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import * as ns from "./fixture.js";
        console.log("first" in ns, "second" in ns, "both" in ns);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true true false\n");
}

#[test]
fn module_namespace_supports_numeric_string_export_names() {
    let tempdir = tempdir().unwrap();
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        var a = 0;
        var b = 1;
        export { a as "0", b as "1" };
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import * as ns from "./fixture.js";
        console.log(
          ns[0],
          Reflect.get(ns, 1),
          ns[2],
          0 in ns,
          Reflect.has(ns, 1),
          2 in ns
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "0 1 undefined true true false\n"
    );
}

#[test]
fn module_namespace_uninitialized_exports_raise_reference_errors() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import * as ns from "./entry.js";
        __ayyAssertThrows(ReferenceError, function() { ns.local1; }, "get");
        __ayyAssertThrows(ReferenceError, function() {
          Object.prototype.hasOwnProperty.call(ns, "local1");
        }, "hasOwnProperty local1");
        __ayyAssertThrows(ReferenceError, function() {
          Object.getOwnPropertyDescriptor(ns, "local1");
        }, "getOwnPropertyDescriptor local1");
        __ayyAssertThrows(ReferenceError, function() {
          Object.prototype.hasOwnProperty.call(ns, "renamed");
        }, "hasOwnProperty renamed");
        __ayyAssertThrows(ReferenceError, function() {
          Object.getOwnPropertyDescriptor(ns, "renamed");
        }, "getOwnPropertyDescriptor renamed");
        __ayyAssertThrows(ReferenceError, function() {
          Object.prototype.hasOwnProperty.call(ns, "indirect");
        }, "hasOwnProperty indirect");
        __ayyAssertThrows(ReferenceError, function() {
          Object.getOwnPropertyDescriptor(ns, "indirect");
        }, "getOwnPropertyDescriptor indirect");
        __ayyAssertThrows(ReferenceError, function() {
          Object.prototype.hasOwnProperty.call(ns, "default");
        }, "hasOwnProperty default");
        __ayyAssertThrows(ReferenceError, function() {
          Object.getOwnPropertyDescriptor(ns, "default");
        }, "getOwnPropertyDescriptor default");
        __ayyAssertThrows(ReferenceError, function() {
          Object.keys(ns);
        }, "keys");
        __ayyAssertThrows(ReferenceError, function() {
          Object.prototype.propertyIsEnumerable.call(ns, "local1");
        }, "propertyIsEnumerable");
        __ayyAssertThrows(ReferenceError, function() {
          for (var key in ns) {
            __ayyFail("enumeration should not reach the loop body");
          }
        }, "enumerate");

        export let local1 = 23;
        let local2 = 45;
        export { local2 as renamed };
        export { local1 as indirect } from "./entry.js";
        export default null;
        console.log("module-uninit-ok");
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "module-uninit-ok\n");
}

#[test]
fn module_namespace_mutation_paths_match_test262_expectations() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import * as ns from "./entry.js";
        export var local1;
        var local2;
        export { local2 as renamed };
        export { local1 as indirect } from "./entry.js";

        console.log(
          Reflect.set(ns, "local1"),
          Reflect.defineProperty(ns, "local1", {}),
          Reflect.defineProperty(ns, "local1", { value: 123 }),
          Reflect.deleteProperty(ns, "local1"),
          Reflect.defineProperty(ns, Symbol.toStringTag, {
            value: "Module",
            writable: false,
            enumerable: false,
            configurable: false
          }),
          Reflect.defineProperty(ns, Symbol.toStringTag, {
            value: "module",
            writable: false,
            enumerable: false,
            configurable: false
          })
        );

        __ayyAssertThrows(TypeError, function() { ns.local1 = null; }, "assign");
        __ayyAssertThrows(TypeError, function() { delete ns.local1; }, "delete");
        __ayyAssertThrows(TypeError, function() {
          Object.defineProperty(ns, "local1", { value: 123 });
        }, "defineProperty");
        __ayyAssertThrows(TypeError, function() { Object.freeze(ns); }, "freeze");
        console.log("module-mutate-ok", Object.isFrozen(ns));
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "false true false false true false\nmodule-mutate-ok false\n"
    );
}

#[test]
fn module_ambiguous_star_exports_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let left = tempdir.path().join("left.js");
    let right = tempdir.path().join("right.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(&left, "export var x = 1;\n").unwrap();
    fs::write(&right, "export var x = 2;\n").unwrap();
    fs::write(
        &fixture,
        r#"
        export * from "./left.js";
        export * from "./right.js";
        "#,
    )
    .unwrap();
    fs::write(&entry, "import { x } from \"./fixture.js\";\n").unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&entry, &options, true).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("ambiguous export `x`"));
    assert!(message.contains("fixture.js"));
}

#[test]
fn module_self_imports_can_observe_later_var_exports() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import { x as y , } from "./entry.js";
        console.log(y);
        export var x = 23;
        console.log(y);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "undefined\n23\n");
}

#[test]
fn module_rejects_malformed_string_export_names_during_compilation() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(&fixture, "export var mercury = 1;\n").unwrap();
    fs::write(
        &entry,
        r#"
        export { "mercury" as "\uD83D" } from "./fixture.js";
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&entry, &options, true).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("malformed module export name"));
}

#[test]
fn module_unresolvable_local_exports_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(&entry, "export { unresolvable };\n").unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&entry, &options, true).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("unresolvable export `unresolvable`"));
}

#[test]
fn module_duplicate_top_level_lexical_functions_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        function x() {}
        async function* x() {}
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&entry, &options, true).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("duplicate lexical name `x`"));
}

#[test]
fn script_duplicate_block_scoped_async_functions_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("input.js");
    let output = tempdir.path().join("input.wasm");

    fs::write(&input, "{ async function f() {} async function f() {} }\n").unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&input, &options, false).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("duplicate lexical name `f`"));
}

#[test]
fn arrow_non_simple_duplicate_parameters_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("input.js");
    let output = tempdir.path().join("input.wasm");

    fs::write(&input, "0, (x = 0, x) => {};\n").unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&input, &options, false).unwrap_err();
    let message = format!("{error:#}");
    assert!(
        message.contains("duplicate parameter name `x`"),
        "{message}"
    );
}

#[test]
fn script_block_lexical_and_var_redeclaration_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("input.js");
    let output = tempdir.path().join("input.wasm");

    fs::write(&input, "function x() { { let f; var f; } }\n").unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&input, &options, false).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("duplicate lexical name `f`"));
}

#[test]
fn module_circular_indirect_reexports_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(&entry, "export { x } from \"./fixture.js\";\n").unwrap();
    fs::write(&fixture, "export { x } from \"./entry.js\";\n").unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&entry, &options, true).unwrap_err();
    assert!(!format!("{error:#}").is_empty());
}

#[test]
fn module_duplicate_import_attribute_keys_fail_during_compilation() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(&fixture, "export default 1;\n").unwrap();
    fs::write(
        &entry,
        r#"
        import value from "./fixture.js" with {
          type: "json",
          "typ\u0065": ""
        };
        console.log(value);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output,
        target: "wasm32-wasip2".to_string(),
    };

    let error = compile_file_with_goal(&entry, &options, true).unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("duplicate import attribute key `type`"));
}

#[test]
fn module_indirect_reexport_imports_can_bind_later_generator_exports() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export { A as B } from "./entry.js";
        export const results = [];
        try {
          A;
        } catch (error) {
          results.push(error.name, typeof A);
        }
        try {
          B;
        } catch (error) {
          results.push(error.name, typeof B);
        }
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import { B, results } from "./fixture.js";
        console.log(B().next().value);
        export function* A() { return 455; }
        console.log(results.length, results[0], results[1], results[2], results[3]);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "455\n4 ReferenceError undefined ReferenceError undefined\n"
    );
}

#[test]
fn module_indirect_reexport_cycles_can_resolve_through_later_exports() {
    let tempdir = tempdir().unwrap();
    let main = tempdir.path().join("main.js");
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("main.wasm");

    fs::write(
        &main,
        "import { a } from \"./entry.js\";\nconsole.log(a);\n",
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        export { a } from "./fixture.js";
        export { c as b } from "./fixture.js";
        export var d = 7;
        "#,
    )
    .unwrap();
    fs::write(
        &fixture,
        r#"
        export { b as a } from "./entry.js";
        export { d as c } from "./entry.js";
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&main, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\n");
}

#[test]
fn module_top_level_await_rejected_promises_surface_first_rejection() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export default 42;
        await Promise.resolve().then(function() {
          return Promise.reject(new RangeError("range"));
        });
        var rejection = Promise.reject(new TypeError("type"));
        await rejection;
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import value from "./fixture.js";
        console.log("unreachable", value);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        !run.status.success(),
        "wasmtime unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("RangeError"), "{stderr}");
    assert!(!stderr.contains("TypeError"), "{stderr}");
    assert!(!String::from_utf8_lossy(&run.stdout).contains("unreachable"));
}

#[test]
fn module_top_level_await_import_rejection_blocks_dependent_module() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let fixture = tempdir.path().join("fixture.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export default 42;
        await Promise.resolve().then(function() {
          return Promise.reject(new RangeError());
        });
        var rejection = Promise.reject(new TypeError());
        await rejection;
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        function Test262Error(message) {
          this.name = "Test262Error";
          this.message = message ?? "";
        }

        import foo from "./fixture.js";

        throw new Test262Error("this should be unreachable");
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        !run.status.success(),
        "wasmtime unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("RangeError"), "{stderr}");
    assert!(!stderr.contains("Test262Error"), "{stderr}");
}

#[test]
fn module_top_level_await_while_dynamic_evaluation_runs_each_then_once() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &input,
        r#"
        var values = [];
        var p = Promise.resolve().then(() => {
          p = Promise.resolve().then(() => {
            p = Promise.resolve().then(() => {
              values.push(3);
              return false;
            });

            values.push(2);
            return true;
          });

          values.push(1);
          return true;
        });

        while (await p) {}

        console.log(values.length, values[0], values[1], values[2]);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "3 1 2 3\n");
}

#[test]
fn module_top_level_await_awaits_thenables() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &input,
        r#"
        var thenable = {
          then: function(resolve, reject) {
            resolve(42);
          }
        };

        console.log(await thenable);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");
}

#[test]
fn module_top_level_await_keeps_sibling_snapshot_before_async_resume() {
    let tempdir = tempdir().unwrap();
    let async_module = tempdir.path().join("async.js");
    let sync_module = tempdir.path().join("sync.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &async_module,
        r#"
        globalThis.check = false;
        await 0;
        globalThis.check = true;
        "#,
    )
    .unwrap();
    fs::write(
        &sync_module,
        r#"
        export const { check } = globalThis;
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import "./async.js";
        import { check } from "./sync.js";

        console.log("check", check, globalThis.check);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "check false true\n");
}

#[test]
fn module_top_level_await_self_import_sees_tick_and_live_default_export() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        var x = "synchronous evaluation";
        Promise.resolve().then(() => x = "tick in the async evaluation");

        import self from "./entry.js";

        console.log("before", x);
        try {
          let value = self;
          console.log("self-before", value);
        } catch (error) {
          console.log("self-before-error", error.name);
        }

        export default await Promise.resolve(42);

        console.log("after", x, self);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "before synchronous evaluation\nself-before-error ReferenceError\nafter tick in the async evaluation 42\n"
    );
}

#[test]
fn module_top_level_await_async_import_resolution_ticks_after_own_await() {
    let tempdir = tempdir().unwrap();
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        await 1;
        await 2;
        export default await Promise.resolve(42);
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        var x = "synchronous evaluation";
        Promise.resolve().then(() => x = "tick in the async evaluation");

        import foo from "./fixture.js";

        console.log("before", foo, x);
        await 1;
        console.log("after", x);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "before 42 synchronous evaluation\nafter tick in the async evaluation\n"
    );
}

#[test]
fn module_top_level_await_new_await_parens_constructs_builtins() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        console.log(
          (new (await Number)).valueOf(),
          (new (await String)).valueOf(),
          (new (await Boolean)).valueOf(),
          (new (await Array)).length,
          (new (await Map)).size,
          (new (await Set)).size
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "0  false 0 0 0\n");
}

#[test]
fn module_export_destructuring_declarations_compile_and_bind() {
    let tempdir = tempdir().unwrap();
    let fixture = tempdir.path().join("fixture.js");
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &fixture,
        r#"
        export const { check } = { check: 7 };
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        import { check } from "./fixture.js";
        console.log(check);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "7\n");
}

#[test]
fn compiles_native_error_objects_with_prototype_names() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("errors.js");
    let output = tempdir.path().join("errors.wasm");

    fs::write(
        &input,
        r#"
        __ayyAssertThrows(TypeError, function() { throw new TypeError("bad"); }, "type");
        __ayyAssertThrows(ReferenceError, function() { throw new ReferenceError("missing"); }, "ref");
        console.log(
          new TypeError().name,
          new ReferenceError().name,
          new RangeError().name,
          Object.getPrototypeOf(new TypeError()) === TypeError.prototype
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, false).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "TypeError ReferenceError RangeError true\n"
    );
}

#[test]
fn compiles_assert_throws_for_asi_unbound_identifier_reads() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("asi-unbound-read.js");
    let output = tempdir.path().join("asi-unbound-read.wasm");

    fs::write(
        &input,
        r#"
        __ayyAssertThrows(ReferenceError, function() {
          var x
          y
        }, "asi reference");

        console.log("ok");
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn catches_unbound_identifier_reads_as_reference_errors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("catch-reference-error.js");
    let output = tempdir.path().join("catch-reference-error.wasm");

    fs::write(
        &input,
        r#"
        try {
          missing;
        } catch (error) {
          console.log("ref", error instanceof ReferenceError);
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ref true\n");
}

#[test]
fn nested_catch_parameters_shadow_outer_catch_parameters() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("nested-catch-shadowing.js");
    let output = tempdir.path().join("nested-catch-shadowing.wasm");

    fs::write(
        &input,
        r#"
        function fn() {
          var c = 1;
          try {
            throw "stuff3";
          } catch (c) {
            try {
              throw "stuff4";
            } catch (c) {
              console.log("inner", c === "stuff4");
              c = 3;
              console.log("inner-set", c === 3);
            }
            console.log("outer", c === "stuff3");
          }
          console.log("var", c === 1);
        }
        fn();
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
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "inner true\ninner-set true\nouter true\nvar true\n"
    );
}

#[test]
fn unhandled_native_error_throws_surface_error_names() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("throw-type-error.js");
    let output = tempdir.path().join("throw-type-error.wasm");

    fs::write(&input, "throw new TypeError('bad');\n").unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, false).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(!run.status.success(), "wasmtime unexpectedly succeeded");
    assert!(String::from_utf8_lossy(&run.stderr).contains("TypeError"));
}

#[test]
fn unhandled_strict_direct_eval_syntax_errors_surface_error_names() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("strict-eval-syntax-error.js");
    let output = tempdir.path().join("strict-eval-syntax-error.wasm");

    fs::write(&input, "\"use strict\";\neval('var public = 1;');\n").unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, false).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(!run.status.success(), "wasmtime unexpectedly succeeded");
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("SyntaxError"), "{stderr}");
    assert!(!stderr.contains("TypeError"), "{stderr}");
}

#[test]
fn compiles_direct_eval_use_strict_overrides_sloppy_assignment_semantics() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("eval-use-strict-override.js");
    let output = tempdir.path().join("eval-use-strict-override.wasm");

    fs::write(
        &input,
        r#"
        function probe() {
          try {
            eval('"use strict"; unresolvable = null;');
            console.log("no");
          } catch (error) {
            console.log(error instanceof ReferenceError);
          }
        }

        probe();
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, false).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n");
}

#[test]
fn compiles_typeof_for_caught_direct_eval_super_errors_stored_in_bindings() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("caught-eval-super-typeof.js");
    let output = tempdir.path().join("caught-eval-super-typeof.wasm");

    fs::write(
        &input,
        r#"
        var caught;
        function f() {
          try {
            eval('super.x;');
          } catch (err) {
            caught = err;
          }
        }

        f();
        console.log(typeof caught, caught.constructor === SyntaxError);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, false).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "object true\n");
}

#[test]
fn compiles_direct_eval_super_property_reads_in_methods() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("eval-super-prop-method.js");
    let output = tempdir.path().join("eval-super-prop-method.wasm");

    fs::write(
        &input,
        r#"
        var superProp = null;
        var o = {
          test262: null,
          method() {
            superProp = eval('super.test262;');
          }
        };

        o.method();
        console.log(superProp === undefined);

        Object.setPrototypeOf(o, { test262: 262 });
        o.method();
        console.log(superProp === 262);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, false).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\ntrue\n");
}

#[test]
fn compiles_direct_eval_this_in_strict_function_callers() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("eval-this-strict-caller.js");
    let output = tempdir.path().join("eval-this-strict-caller.wasm");

    fs::write(
        &input,
        r#"
        "use strict";
        var thisValue = null;

        (function() {
          thisValue = eval('this;');
        }());

        console.log(thisValue === undefined);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, false).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n");
}

#[test]
fn module_namespace_reflect_define_property_covers_nonexistent_and_exact_cases() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &entry,
        r#"
        import * as ns from './entry.js';
        export var local1;
        var local2;
        export { local2 as renamed };
        export { local1 as indirect } from './entry.js';
        var sym = Symbol('test262');

        console.log(
          typeof Reflect.defineProperty,
          Reflect.defineProperty(ns, 'local2', {}),
          Reflect.defineProperty(ns, 0, {}),
          Reflect.defineProperty(ns, sym, {}),
          Reflect.defineProperty(ns, Symbol.iterator, {}),
          Reflect.defineProperty(ns, 'local1', {}),
          Reflect.defineProperty(ns, 'renamed', {}),
          Reflect.defineProperty(ns, 'indirect', {}),
          Reflect.defineProperty(ns, Symbol.toStringTag, {}),
          Reflect.defineProperty(ns, 'local1', { value: 123 }),
          Reflect.defineProperty(ns, 'renamed', { value: 123 }),
          Reflect.defineProperty(ns, 'indirect', { value: 123 }),
          Reflect.defineProperty(ns, Symbol.toStringTag, {
            value: 'module',
            writable: false,
            enumerable: false,
            configurable: false
          }),
          Reflect.defineProperty(ns, 'indirect', {
            writable: true,
            enumerable: true,
            configurable: false
          }),
          Reflect.defineProperty(ns, 'indirect', {
            writable: true,
            enumerable: true,
            configurable: true
          }),
          Reflect.defineProperty(ns, Symbol.toStringTag, {
            value: 'Module',
            writable: false,
            enumerable: false,
            configurable: false
          })
        );
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "function false false false false true true true true false false false false true false true\n"
    );
}

#[test]
fn compiles_array_literal_and_call_spread() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("spread.js");
    let output = tempdir.path().join("spread.wasm");

    fs::write(
        &input,
        r#"
        function join3(a, b, c) {
          return a + "-" + b + "-" + c;
        }

        let base = [1, 2];
        let combined = [...base, 3];
        console.log(join3(...combined), combined.length, combined[2]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1-2-3 3 3\n");
}

#[test]
fn module_dynamic_import_starts_dynamic_module_on_demand() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let dep = tempdir.path().join("dep.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &dep,
        r#"
        console.log("dep");
        export default 42;
        export var x = "named";
        export var y = 39;
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        console.log("before");
        var ns = await import("./dep.js");
        console.log("after", ns.default, ns.x, ns.y);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "before\ndep\nafter 42 named 39\n"
    );
}

#[test]
fn module_dynamic_import_reuses_single_module_start() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let dep = tempdir.path().join("dep.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &dep,
        r#"
        await Promise.resolve().then(() => console.log("dep-tick"));
        export default 7;
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        let first = import("./dep.js");
        let second = import("./dep.js");
        let a = await first;
        let b = await second;
        console.log("done", a.default, b.default);
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "dep-tick\ndone 7 7\n");
}

#[test]
fn module_dynamic_import_propagates_rejection() {
    let tempdir = tempdir().unwrap();
    let entry = tempdir.path().join("entry.js");
    let dep = tempdir.path().join("dep.js");
    let output = tempdir.path().join("entry.wasm");

    fs::write(
        &dep,
        r#"
        throw new TypeError("boom");
        "#,
    )
    .unwrap();
    fs::write(
        &entry,
        r#"
        try {
          await import("./dep.js");
          console.log("bad");
        } catch (error) {
          console.log(error.name);
        }
        "#,
    )
    .unwrap();

    let options = CompileOptions {
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&entry, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "TypeError\n");
}

#[test]
fn object_literal_proto_initializer_sets_super_base() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("object-proto-super.js");
    let output = tempdir.path().join("object-proto-super.wasm");

    fs::write(
        &input,
        r#"
        var result = "";
        var proto = { set p(v) { result = "ok"; } };
        var proto2 = { set p(v) { result = "bad"; } };
        var obj = {
          __proto__: proto,
          m() {
            super[key] = 10;
          }
        };
        var key = {
          toString() {
            Object.setPrototypeOf(obj, proto2);
            return "p";
          }
        };
        obj.m();
        console.log(result);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn custom_constructor_instances_keep_constructor_identity() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("custom-constructor.js");
    let output = tempdir.path().join("custom-constructor.wasm");

    fs::write(
        &input,
        r#"
        function DummyError() {}
        let error = new DummyError();
        console.log(error.constructor === DummyError);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n");
}

#[test]
fn thrown_custom_constructor_instances_keep_constructor_identity() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("thrown-custom-constructor.js");
    let output = tempdir.path().join("thrown-custom-constructor.wasm");

    fs::write(
        &input,
        r#"
        function DummyError() {}
        function thrower() {
          throw new DummyError();
        }
        try {
          thrower();
        } catch (error) {
          console.log(error.constructor === DummyError);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n");
}

#[test]
fn super_computed_assignment_short_circuits_rhs_on_property_throw() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("super-computed-short-circuit.js");
    let output = tempdir.path().join("super-computed-short-circuit.wasm");

    fs::write(
        &input,
        r#"
        function DummyError() {}
        var prop = function() { throw new DummyError(); };
        var expr = function() { console.log("rhs"); throw new Error("rhs"); };
        class C extends class {} {
          m() {
            super[prop()] = expr();
          }
        }
        try {
          (new C()).m();
        } catch (error) {
          console.log(error.constructor === DummyError);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n");
}

#[test]
fn super_spread_propagates_iterator_and_reference_errors() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("super-spread-errors.js");
    let output = tempdir.path().join("super-spread-errors.wasm");

    fs::write(
        &input,
        r#"
        class Parent {
          constructor() {}
        }

        class MissingRefChild extends Parent {
          constructor() {
            super(...unresolvableReference);
          }
        }

        class IteratorChild extends Parent {
          constructor() {
            super(...function* () {
              throw new TypeError("boom");
            }());
          }
        }

        try {
          new MissingRefChild();
          console.log("bad-ref");
        } catch (error) {
          console.log("ref", error.name);
        }

        try {
          new IteratorChild();
          console.log("bad-iter");
        } catch (error) {
          console.log("iter", error.name);
        }
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "ref ReferenceError\niter TypeError\n"
    );
}

#[test]
fn async_super_method_body_preserves_this_binding() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("async-super-method-body.js");
    let output = tempdir.path().join("async-super-method-body.wasm");

    fs::write(
        &input,
        r#"
        class A {
          async method() {
            return this.value;
          }
        }

        class B extends A {
          constructor() {
            super();
            this.value = "sup";
          }

          async method() {
            var x = await super.method();
            console.log("x", x);
          }
        }

        new B().method().then(
          function(value) { console.log("done", value); },
          function(error) { console.log("err", error && error.name, "" + error); }
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "x sup\ndone undefined\n"
    );
}

#[test]
fn async_await_of_immediately_resolved_values_runs_without_trapping() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("async-await-immediate.js");
    let output = tempdir.path().join("async-await-immediate.wasm");

    fs::write(
        &input,
        r#"
        async function f() {
          return "sup";
        }

        async function g() {
          let a = await "lhs";
          let b = await Promise.resolve("rhs");
          let c = await f();
          console.log(a, b, c);
        }

        g().then(
          function(value) { console.log("done", value); },
          function(error) { console.log("err", error && error.name, "" + error); }
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "lhs rhs sup\ndone undefined\n"
    );
}

#[test]
fn object_spread_keeps_symbol_values_and_own_key_order() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("object-spread-symbols.js");
    let output = tempdir.path().join("object-spread-symbols.wasm");

    fs::write(
        &input,
        r#"
        let calls = [];
        let sym = Symbol("foo");
        let source = { get z() { calls.push("z"); }, get a() { calls.push("a"); } };
        Object.defineProperty(source, 1, { get() { calls.push(1); return "one"; }, enumerable: true });
        Object.defineProperty(source, sym, { enumerable: true, value: sym });

        let copy = { ...source, extra: 1 };

        console.log(calls[0], calls[1], calls[2]);
        console.log(copy[sym].toString());
        let keys = Object.keys(copy);
        console.log(keys[0], keys[1], keys[2], keys[3]);
        console.log(Object.is(copy.extra, 1), Object.is(copy, source));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "1 z a\nSymbol(foo)\n1 z a extra\ntrue false\n"
    );
}

#[test]
fn compiles_arrow_function_restricted_caller_and_arguments_properties() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arrow-restricted-properties.js");
    let output = tempdir.path().join("arrow-restricted-properties.wasm");

    fs::write(
        &input,
        r#"
        var arrowFn = () => {};

        console.log(
          "own",
          arrowFn.hasOwnProperty("caller"),
          arrowFn.hasOwnProperty("arguments")
        );

        __ayyAssertThrows(TypeError, function() {
          return arrowFn.caller;
        }, "get-caller");

        __ayyAssertThrows(TypeError, function() {
          arrowFn.caller = {};
        }, "set-caller");

        __ayyAssertThrows(TypeError, function() {
          return arrowFn.arguments;
        }, "get-arguments");

        __ayyAssertThrows(TypeError, function() {
          arrowFn.arguments = {};
        }, "set-arguments");

        console.log("throws");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "own false false\nthrows\n"
    );
}

#[test]
fn compiles_function_restricted_caller_and_arguments_properties() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("function-restricted-properties.js");
    let output = tempdir.path().join("function-restricted-properties.wasm");

    fs::write(
        &input,
        r#"
        function f() {}

        console.log(
          "own",
          f.hasOwnProperty("caller"),
          f.hasOwnProperty("arguments")
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "own false false\n");
}

#[test]
fn compiles_async_generator_method_next_promise_once_with_receiver_this() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("async-generator-method-next-once.js");
    let output = tempdir.path().join("async-generator-method-next-once.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;

        class C {
          async *method() {
            console.log("own", this.method.hasOwnProperty("arguments"));
            callCount++;
          }
        }

        var iter = C.prototype.method();
        console.log("created", callCount);
        iter.next().then(function(v) {
          console.log("done", v.done, callCount);
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "created 0\nown false\ndone true 1\n"
    );
}

#[test]
fn compiles_async_generator_method_next_then_chain_with_function_handler() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-method-next-then-chain-function.js");
    let output = tempdir
        .path()
        .join("async-generator-method-next-then-chain-function.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;
        function done(value) {
          console.log("done", value === undefined ? "undefined" : value);
        }

        class C {
          async *method() {
            console.log("own", this.method.hasOwnProperty("caller"));
            callCount++;
          }
        }

        C.prototype.method().next()
          .then(function() {
            console.log("then1", callCount);
          }, done)
          .then(done, done);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "own false\nthen1 1\ndone undefined\n"
    );
}

#[test]
fn compiles_async_generator_rejected_yield_next_catch_chain() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-rejected-yield-next-catch-chain.js");
    let output = tempdir
        .path()
        .join("async-generator-rejected-yield-next-catch-chain.wasm");

    fs::write(
        &input,
        r#"
        let error = new Error("boom");

        async function* gen() {
          console.log("enter");
          yield Promise.reject(error);
          console.log("after");
        }

        var iter = gen();
        iter.next()
          .then(function() {
            console.log("resolved");
          })
          .catch(function(err) {
            console.log("caught", err === error, err.message);
            iter.next().then(function(result) {
              console.log("next2", result.done, result.value);
            });
          });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "enter\ncaught true undefined\nnext2 true undefined\n"
    );
}

#[test]
fn compiles_for_await_async_generator_rejection_then_completion() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("for-await-async-generator-rejection-then-completion.js");
    let output = tempdir
        .path()
        .join("for-await-async-generator-rejection-then-completion.wasm");

    fs::write(
        &input,
        r#"
        let error = new Error("boom");

        async function* readFile() {
          yield Promise.reject(error);
          yield "unreachable";
        }

        class C {
          async *gen() {
            for await (let line of readFile()) {
              yield line;
            }
            console.log("after");
          }
        }

        var iter = C.prototype.gen();
        iter.next().then(
          function() {
            throw new Error("resolved");
          },
          function(rejectValue) {
            console.log("caught", rejectValue === error, rejectValue.message);
            iter.next().then(function({done, value}) {
              console.log("next2", done, value);
            });
          }
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "caught true undefined\nnext2 true undefined\n"
    );
}

#[test]
fn compiles_for_await_sync_iterable_rejection_then_completion() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("for-await-sync-iterable-rejection-then-completion.js");
    let output = tempdir
        .path()
        .join("for-await-sync-iterable-rejection-then-completion.wasm");

    fs::write(
        &input,
        r#"
        let error = new Error("boom");
        let iterable = [
          Promise.reject(error),
          "unreachable"
        ];

        class C {
          async *gen() {
            for await (let value of iterable) {
              yield value;
            }
            console.log("after");
          }
        }

        var iter = C.prototype.gen();
        iter.next().then(
          function() {
            throw new Error("resolved");
          },
          function(rejectValue) {
            console.log("caught", rejectValue === error, rejectValue.message);
            iter.next().then(function({done, value}) {
              console.log("next2", done, value);
            });
          }
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "caught true undefined\nnext2 true undefined\n"
    );
}

#[test]
fn compiles_direct_function_expression_object_destructuring_call() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("direct-function-expression-object-destructuring-call.js");
    let output = tempdir
        .path()
        .join("direct-function-expression-object-destructuring-call.wasm");

    fs::write(
        &input,
        r#"
        (function({done, value}) {
          console.log("direct", done, value);
        })({done: true, value: undefined});
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "direct true undefined\n"
    );
}

#[test]
fn compiles_returned_arrow_closures_capturing_with_bindings() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arrow-with-capture.js");
    let output = tempdir.path().join("arrow-with-capture.wasm");

    fs::write(
        &input,
        r#"
        function foo(value) {
          var scope = { a: value };
          with (scope) {
            return () => a;
          }
        }

        var stored = foo(10);

        console.log("stored", stored());
        console.log("direct", foo(30)());
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "stored 10\ndirect 30\n"
    );
}

#[test]
fn compiles_array_for_each_arrow_callbacks_with_lexical_this() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("arrow-foreach-thisarg.js");
    let output = tempdir.path().join("arrow-foreach-thisarg.wasm");

    fs::write(
        &input,
        r#"
        var calls = 0;
        var usurper = {};

        [1].forEach(value => {
          calls++;
          __assertNotSameValue(this, usurper, "lexical-this");
        }, usurper);

        console.log("calls", calls);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "calls 1\n");
}

#[test]
fn compiles_array_destructuring_parameters_from_arrays_and_custom_iterators() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("array-destructuring-params.js");
    let output = tempdir.path().join("array-destructuring-params.wasm");

    fs::write(
        &input,
        r#"
        var closeCount = 0;
        var iterable = {};
        iterable[Symbol.iterator] = function() {
          return {
            next: function() {
              return { value: 9, done: false };
            },
            return: function() {
              closeCount += 1;
              return {};
            }
          };
        };

        function plain([x, y, z]) {
          console.log("plain", x, y, z);
        }

        var arrow = ([value]) => {
          console.log("arrow", value, closeCount);
        };

        plain([1, 2, 3]);
        arrow(iterable);
        console.log("close", closeCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "plain 1 2 3\narrow 9 1\nclose 1\n"
    );
}

#[test]
fn compiles_nested_array_destructuring_parameter_defaults() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("nested-array-destructuring-param-default.js");
    let output = tempdir
        .path()
        .join("nested-array-destructuring-param-default.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;
        var f;
        f = ([[x, y, z] = [4, 5, 6]]) => {
          console.log("vals", x, y, z);
          callCount = callCount + 1;
        };

        f([]);
        console.log("count", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "vals 4 5 6\ncount 1\n"
    );
}

#[test]
fn compiles_nested_array_destructuring_default_generators() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("nested-array-destructuring-defaults-generator.js");
    let output = tempdir
        .path()
        .join("nested-array-destructuring-defaults-generator.wasm");

    fs::write(
        &input,
        r#"
        var first = 0;
        var second = 0;
        function* g() {
          first += 1;
          yield;
          second += 1;
        }

        var f = ([[,] = g()]) => {
          console.log("gen", first, second);
        };

        f([]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "gen 1 0\n");
}

#[test]
fn compiles_nested_array_destructuring_rest_copies_arrays() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("nested-array-destructuring-rest-copy.js");
    let output = tempdir
        .path()
        .join("nested-array-destructuring-rest-copy.wasm");

    fs::write(
        &input,
        r#"
        var values = [2, 1, 3];

        var f = ([[...x] = values]) => {
          console.log("rest-copy", Array.isArray(x), x[0], x[1], x[2], x.length, Object.is(x, values));
        };

        f([]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "rest-copy 1 2 1 3 3 false\n"
    );
}

#[test]
fn compiles_nested_array_destructuring_rest_array_elements() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("nested-array-destructuring-rest-elements.js");
    let output = tempdir
        .path()
        .join("nested-array-destructuring-rest-elements.wasm");

    fs::write(
        &input,
        r#"
        var f = ([...[x, y, z]]) => {
          console.log("vals", x, y, z);
        };

        f([3, 4, 5]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "vals 3 4 5\n");
}

#[test]
fn compiles_arrow_destructuring_with_array_prototype_iterator_override() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("arrow-destructuring-array-prototype-iterator.js");
    let output = tempdir
        .path()
        .join("arrow-destructuring-array-prototype-iterator.wasm");

    fs::write(
        &input,
        r#"
        Array.prototype[Symbol.iterator] = function* () {
          if (this.length > 0) {
            yield this[0];
          }
          if (this.length > 1) {
            yield this[1];
          }
          if (this.length > 2) {
            yield 42;
          }
        };

        var f = ([x, y, z]) => {
          console.log("vals", x, y, z);
        };

        f([1, 2, 3]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "vals 1 2 42\n");
}

#[test]
fn compiles_arrow_default_destructuring_with_array_prototype_iterator_override() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("arrow-default-destructuring-array-prototype-iterator.js");
    let output = tempdir
        .path()
        .join("arrow-default-destructuring-array-prototype-iterator.wasm");

    fs::write(
        &input,
        r#"
        Array.prototype[Symbol.iterator] = function* () {
          if (this.length > 0) {
            yield this[0];
          }
          if (this.length > 1) {
            yield this[1];
          }
          if (this.length > 2) {
            yield 42;
          }
        };

        var f = ([x, y, z] = [1, 2, 3]) => {
          console.log("vals", x, y, z);
        };

        f();
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "vals 1 2 42\n");
}

#[test]
fn compiles_class_method_destructuring_with_array_prototype_iterator_override() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-method-destructuring-array-prototype-iterator.js");
    let output = tempdir
        .path()
        .join("class-method-destructuring-array-prototype-iterator.wasm");

    fs::write(
        &input,
        r#"
        Array.prototype[Symbol.iterator] = function* () {
          if (this.length > 0) {
            yield this[0];
          }
          if (this.length > 1) {
            yield this[1];
          }
          if (this.length > 2) {
            yield 42;
          }
        };

        class C {
          method([x, y, z]) {
            console.log("vals", x, y, z);
          }
        }

        new C().method([1, 2, 3]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "vals 1 2 42\n");
}

#[test]
fn compiles_class_async_generator_destructuring_with_array_prototype_iterator_override() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-async-generator-array-prototype-iterator.js");
    let output = tempdir
        .path()
        .join("class-async-generator-array-prototype-iterator.wasm");

    fs::write(
        &input,
        r#"
        Array.prototype[Symbol.iterator] = function* () {
          if (this.length > 0) {
            yield this[0];
          }
          if (this.length > 1) {
            yield this[1];
          }
          if (this.length > 2) {
            yield 42;
          }
        };

        class C {
          async *method([x, y, z]) {
            console.log("vals", x, y, z);
          }
        }

        let iterator = new C().method([1, 2, 3]);
        iterator.next().then(function () {
          console.log("done");
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "vals 1 2 42\ndone\n");
}

#[test]
fn compiles_arrow_elision_over_throwing_generator_iterators() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("arrow-elision-throwing-generator-iterator.js");
    let output = tempdir
        .path()
        .join("arrow-elision-throwing-generator-iterator.wasm");

    fs::write(
        &input,
        r#"
        var following = 0;
        var iter = function* () {
          throw 123;
          following += 1;
        }();

        var f = ([,]) => {};

        try {
          f(iter);
        } catch (error) {
          console.log("caught", error);
        }

        iter.next();
        console.log("after", following);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "caught 123\nafter 0\n"
    );
}

#[test]
fn compiles_nested_array_destructuring_default_arrow_function_names() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("nested-array-destructuring-default-arrow-name.js");
    let output = tempdir
        .path()
        .join("nested-array-destructuring-default-arrow-name.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;
        var f;
        f = ([arrow = () => {}]) => {
          console.log("name", arrow.name);
          callCount += 1;
        };

        f([]);
        console.log("count", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "name arrow\ncount 1\n"
    );
}

#[test]
fn compiles_nested_array_destructuring_default_class_names() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("nested-array-destructuring-default-class-name.js");
    let output = tempdir
        .path()
        .join("nested-array-destructuring-default-class-name.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;
        var f;
        f = ([cls = class {}, xCls = class X {}, xCls2 = class { static name() {} }]) => {
          console.log(
            "checks",
            cls.name == "cls",
            xCls.name != "xCls",
            xCls2.name != "xCls2",
            typeof xCls2.name
          );
          callCount += 1;
        };

        f([]);
        console.log("count", callCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "checks true true true function\ncount 1\n"
    );
}

#[test]
fn compiles_captured_class_static_async_generator_default_throw() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("captured-class-static-async-generator-default-throw.js");
    let output = tempdir
        .path()
        .join("captured-class-static-async-generator-default-throw.wasm");

    fs::write(
        &input,
        r#"
        var callCount = 0;

        class C {
          static async *method(_ = (function() { throw 1; }())) {
            callCount = callCount + 1;
          }
        }

        function invoke() {
          C.method();
        }

        invoke();
        console.log("after", callCount, typeof C.prototype, typeof C.method);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "after 0 object function\n"
    );
}

#[test]
fn compiles_async_generator_sync_iterator_typeerror_rejection() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-sync-iterator-typeerror-rejection.js");
    let output = tempdir
        .path()
        .join("async-generator-sync-iterator-typeerror-rejection.wasm");

    fs::write(
        &input,
        r#"
        var obj = {
          [Symbol.iterator]: {}
        };

        class C {
          static async *gen() {
            yield* obj;
          }
        }

        var iter = C.gen();
        iter.next().then(
          function() { console.log("fulfilled"); },
          function(err) { console.log("rejected", err.constructor === TypeError); }
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "rejected true\n");
}

#[test]
fn compiles_function_prototype_call_results_with_returned_getter_objects() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("function-prototype-call-returned-getter-object.js");
    let output = tempdir
        .path()
        .join("function-prototype-call-returned-getter-object.wasm");

    fs::write(
        &input,
        r#"
        var nextCount = 0;
        var iter = {
          get next() {
            return function() {
              nextCount += 1;
              return {
                get value() {
                  return "x";
                },
                get done() {
                  return false;
                }
              };
            };
          }
        };

        var next = iter.next;
        var step = next.call(iter, "ignored");
        console.log("closure-call", step.value === "x", step.done === false, nextCount);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "closure-call true true 1\n"
    );
}

#[test]
fn compiles_async_generator_yield_star_sync_iterator_next_with_getter_results() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-yield-star-sync-iterator-next-getters.js");
    let output = tempdir
        .path()
        .join("async-generator-yield-star-sync-iterator-next-getters.wasm");

    fs::write(
        &input,
        r#"
        var log = [];
        var obj = {
          get [Symbol.iterator]() {
            log.push("get-iter");
            return function() {
              return {
                get next() {
                  log.push("get-next");
                  return function() {
                    return {
                      get value() {
                        log.push("get-value");
                        return "next-value-1";
                      },
                      get done() {
                        log.push("get-done");
                        return false;
                      }
                    };
                  };
                }
              };
            };
          },
          get [Symbol.asyncIterator]() {
            log.push("get-async");
            return null;
          }
        };

        class C {
          static async *gen() {
            yield* obj;
          }
        }

        var iter = C.gen();
        iter.next().then(function(v) {
          console.log("yield-star", v.value, v.done, log.length);
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "yield-star next-value-1 false 5\n"
    );
}

#[test]
fn compiles_async_generator_yield_star_async_iterator_next_callback() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-yield-star-async-iterator-next.js");
    let output = tempdir
        .path()
        .join("async-generator-yield-star-async-iterator-next.wasm");

    fs::write(
        &input,
        r#"
        var log = [];
        var obj = {
          [Symbol.asyncIterator]() {
            return {
              get next() {
                log.push("get next");
                return function() {
                  log.push("call next");
                  return { value: "next-value-1", done: false };
                };
              }
            };
          }
        };

        class C {
          async *gen() {
            log.push("before yield*");
            yield* obj;
          }
        }

        var iter = C.prototype.gen();
        console.log("start");
        iter.next().then(function(v) {
          console.log("next", v.value, v.done, log.length);
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "start\nnext next-value-1 false 3\n"
    );
}

#[test]
fn compiles_async_generator_yield_star_async_iterator_return_callback() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-yield-star-async-iterator-return.js");
    let output = tempdir
        .path()
        .join("async-generator-yield-star-async-iterator-return.wasm");

    fs::write(
        &input,
        r#"
        var log = [];
        var obj = {
          [Symbol.asyncIterator]() {
            var returnCount = 0;
            return {
              get next() {
                log.push("get next");
                return function() {
                  return { value: "next-value-1", done: false };
                };
              },
              get return() {
                log.push("get return");
                return function(arg) {
                  log.push("call return:" + arg);
                  returnCount++;
                  if (returnCount === 1) {
                    return {
                      get then() {
                        log.push("get return then (1)");
                        return function(resolve) {
                          log.push("call return then (1)");
                          resolve({
                            get value() {
                              log.push("get return value (1)");
                              return "return-value-1";
                            },
                            get done() {
                              log.push("get return done (1)");
                              return false;
                            }
                          });
                        };
                      }
                    };
                  }
                  return {
                    get then() {
                      log.push("get return then (2)");
                      return function(resolve) {
                        log.push("call return then (2)");
                        resolve({
                          get value() {
                            log.push("get return value (2)");
                            return "return-value-2";
                          },
                          get done() {
                            log.push("get return done (2)");
                            return true;
                          }
                        });
                      };
                    }
                  };
                };
              }
            };
          }
        };

        class C {
          async *gen() {
            log.push("before yield*");
            yield* obj;
          }
        }

        var iter = C.prototype.gen();
        console.log("start");
        iter.next().then(function(v) {
          console.log("next", v.value, v.done, log.length);
          iter.return("return-arg-1").then(function(v2) {
            console.log("return1", v2.value, v2.done, log.length);
            iter.return("return-arg-2").then(function(v3) {
              console.log("return2", v3.value, v3.done, log.length);
            });
          });
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "start\nnext next-value-1 false 2\nreturn1 return-value-1 false 8\nreturn2 return-value-2 true 14\n"
    );
}

#[test]
fn compiles_async_generator_yield_star_sync_iterator_return_same_value_callback() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-yield-star-sync-iterator-return-same-value.js");
    let output = tempdir
        .path()
        .join("async-generator-yield-star-sync-iterator-return-same-value.wasm");

    fs::write(
        &input,
        r#"
        function sameValue(left, right, message) {
          if (left === right) {
            return;
          }
          if (left !== left && right !== right) {
            return;
          }
          throw new Error(message);
        }

        var log = [];
        var obj = {
          [Symbol.iterator]() {
            var returnCount = 0;
            return {
              name: "syncIterator",
              get next() {
                log.push({ name: "get next" });
                return function() {
                  return { value: "next-value-1", done: false };
                };
              },
              get return() {
                log.push({ name: "get return", thisValue: this });
                return function() {
                  log.push({
                    name: "call return",
                    thisValue: this,
                    args: [...arguments]
                  });
                  returnCount++;
                  if (returnCount === 1) {
                    return {
                      name: "return-result-1",
                      get value() {
                        log.push({ name: "get return value (1)", thisValue: this });
                        return "return-value-1";
                      },
                      get done() {
                        log.push({ name: "get return done (1)", thisValue: this });
                        return false;
                      }
                    };
                  }
                  return {
                    name: "return-result-2",
                    get value() {
                      log.push({ name: "get return value (2)", thisValue: this });
                      return "return-value-2";
                    },
                    get done() {
                      log.push({ name: "get return done (2)", thisValue: this });
                      return true;
                    }
                  };
                };
              }
            };
          }
        };

        class C {
          async *gen() {
            log.push({ name: "before yield*" });
            yield* obj;
          }
        }

        var iter = C.prototype.gen();
        iter.next().then(function(v) {
          sameValue(v.value, "next-value-1", "next value");
          sameValue(v.done, false, "next done");
          iter.return("return-arg-1").then(function(v2) {
            sameValue(v2.value, "return-value-1", "return1 value");
            sameValue(v2.done, false, "return1 done");
            iter.return().then(function(v3) {
              console.log("pre", log[7].args[0] === undefined);
              sameValue(log[7].args[0], undefined, "return args[0]");
              console.log("done", v3.value, v3.done, log.length);
            });
          });
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "pre true\ndone return-value-2 true 10\n"
    );
}

#[test]
fn compiles_async_generator_yield_star_async_iterator_throw_callback() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("async-generator-yield-star-async-iterator-throw.js");
    let output = tempdir
        .path()
        .join("async-generator-yield-star-async-iterator-throw.wasm");

    fs::write(
        &input,
        r#"
        var log = [];
        var obj = {
          [Symbol.asyncIterator]() {
            var throwCount = 0;
            return {
              get next() {
                log.push("get next");
                return function() {
                  return { value: "next-value-1", done: false };
                };
              },
              get throw() {
                log.push("get throw");
                return function(arg) {
                  log.push("call throw:" + arg);
                  throwCount++;
                  if (throwCount === 1) {
                    return {
                      get then() {
                        log.push("get throw then (1)");
                        return function(resolve) {
                          log.push("call throw then (1)");
                          resolve({
                            get value() {
                              log.push("get throw value (1)");
                              return "throw-value-1";
                            },
                            get done() {
                              log.push("get throw done (1)");
                              return false;
                            }
                          });
                        };
                      }
                    };
                  }
                  return {
                    get then() {
                      log.push("get throw then (2)");
                      return function(resolve) {
                        log.push("call throw then (2)");
                        resolve({
                          get value() {
                            log.push("get throw value (2)");
                            return "throw-value-2";
                          },
                          get done() {
                            log.push("get throw done (2)");
                            return true;
                          }
                        });
                      };
                    }
                  };
                };
              }
            };
          }
        };

        class C {
          async *gen() {
            log.push("before yield*");
            var v = yield* obj;
            log.push("after yield*:" + v);
            return "return-value";
          }
        }

        var iter = C.prototype.gen();
        console.log("start");
        iter.next().then(function(v) {
          console.log("next", v.value, v.done, log.length);
          iter.throw("throw-arg-1").then(function(v2) {
            console.log("throw1", v2.value, v2.done, log.length);
            iter.throw("throw-arg-2").then(function(v3) {
              console.log("throw2", v3.value, v3.done, log.length, log[14]);
            });
          });
        });
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "start\nnext next-value-1 false 2\nthrow1 throw-value-1 false 8\nthrow2 return-value true 15 after yield*:throw-value-2\n"
    );
}

#[test]
fn compiles_class_field_computed_name_abrupt_completion() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-field-computed-name-abrupt.js");
    let output = tempdir.path().join("class-field-computed-name-abrupt.wasm");

    fs::write(
        &input,
        r#"
        function f() {
          throw "boom";
        }

        let caught = false;
        try {
          class C {
            [f()]
          }
        } catch (error) {
          caught = true;
        }

        console.log(caught);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "true\n");
}

#[test]
fn compiles_class_accessor_computed_numeric_name() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-accessor-computed-numeric-name.js");
    let output = tempdir
        .path()
        .join("class-accessor-computed-numeric-name.wasm");

    fs::write(
        &input,
        r#"
        class C {
          get [1 + 1]() {
            return 2;
          }

          set [1 + 1](v) {
            return 2;
          }

          static get [1 + 1]() {
            return 2;
          }

          static set [1 + 1](v) {
            return 2;
          }

          get [1 - 1]() {
            return 0;
          }

          set [1 - 1](v) {
            return 0;
          }

          static get [1 - 1]() {
            return 0;
          }

          static set [1 - 1](v) {
            return 0;
          }
        }

        let c = new C();
        console.log(
          c[2], c[2] = 2, C[2], C[2] = 2,
          c[String(1 + 1)], c[String(1 + 1)] = 2, C[String(1 + 1)], C[String(1 + 1)] = 2,
          c[0], c[0] = 0, C[0], C[0] = 0,
          c[String(1 - 1)], c[String(1 - 1)] = 0, C[String(1 - 1)], C[String(1 - 1)] = 0
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "2 2 2 2 2 2 2 2 0 0 0 0 0 0 0 0\n",
    );
}

#[test]
fn compiles_class_static_numeric_super_member_access() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-static-numeric-super-member-access.js");
    let output = tempdir
        .path()
        .join("class-static-numeric-super-member-access.wasm");

    fs::write(
        &input,
        r#"
        class B {
          static 4() { return 4; }
          static get 5() { return 5; }
        }

        class C extends B {
          static 4() { return super[4](); }
          static get 5() { return super[5]; }
        }

        console.log(C[4](), C[5]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "4 5\n");
}

#[test]
fn compiles_class_accessor_descriptor_shape() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-accessor-descriptor-shape.js");
    let output = tempdir.path().join("class-accessor-descriptor-shape.wasm");

    fs::write(
        &input,
        r#"
        function logDescriptorShape(object, name) {
          var desc = Object.getOwnPropertyDescriptor(object, name);
          console.log(
            desc.configurable | 0,
            desc.enumerable | 0,
            typeof desc.get,
            typeof desc.set,
            ('prototype' in desc.get) | 0,
            ('prototype' in desc.set) | 0
          );
        }

        class C {
          get x() { return this._x; }
          set x(v) { this._x = v; }
          static get staticX() { return this._x; }
          static set staticX(v) { this._x = v; }
        }

        logDescriptorShape(C.prototype, "x");
        logDescriptorShape(C, "staticX");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "1 0 function function 0 0\n1 0 function function 0 0\n",
    );
}

#[test]
fn compiles_verify_property_like_class_setter_descriptor_checks() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("verify-property-like-class-setter-descriptor-checks.js");
    let output = tempdir
        .path()
        .join("verify-property-like-class-setter-descriptor-checks.wasm");

    fs::write(
        &input,
        r#"
        function Test262Error(message) {
          this.name = "Test262Error";
          this.message = message ?? "";
        }

        var __getOwnPropertyDescriptor = Object.getOwnPropertyDescriptor;
        var __getOwnPropertyNames = Object.getOwnPropertyNames;
        var __hasOwnProperty = Function.prototype.call.bind(Object.prototype.hasOwnProperty);

        function verifyProperty(obj, name, expected) {
          var originalDesc = __getOwnPropertyDescriptor(obj, name);
          var names = __getOwnPropertyNames(expected);
          if (names.length !== 1 || names[0] !== "enumerable") throw 1;
          return originalDesc;
        }

        function assertSetterDescriptor(object, name) {
          var desc = verifyProperty(object, name, {
            enumerable: false
          });
          console.log(
            typeof desc.set,
            ("prototype" in desc.set) | 0,
            desc.get === undefined
          );
        }

        class C {
          set x(v) { this._x = v; }
        }

        assertSetterDescriptor(C.prototype, "x");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "function 0 true\n");
}

#[test]
fn compiles_class_prototype_property_descriptor_shape() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-prototype-property-descriptor-shape.js");
    let output = tempdir
        .path()
        .join("class-prototype-property-descriptor-shape.wasm");

    fs::write(
        &input,
        r#"
        class C {}
        var descr = Object.getOwnPropertyDescriptor(C, "prototype");
        console.log(
          (descr.configurable === false) | 0,
          (descr.enumerable === false) | 0,
          (descr.writable === false) | 0,
          (descr.value === C.prototype) | 0
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1 1 1 1\n");
}

#[test]
fn compiles_derived_class_super_constructor_prototype_wiring() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("derived-class-super-constructor-wiring.js");
    let output = tempdir
        .path()
        .join("derived-class-super-constructor-wiring.wasm");

    fs::write(
        &input,
        r#"
        class Base {
          constructor(x) {
            this.foobar = x;
          }
        }

        class Subclass extends Base {
          constructor(x) {
            super(x);
          }
        }

        let callType = false;
        try {
          Subclass(1);
        } catch (error) {
          callType = error.constructor === TypeError;
        }

        let s = new Subclass(1);
        console.log(s.foobar, Object.getPrototypeOf(s) === Subclass.prototype, callType);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1 true true\n");
}

#[test]
fn compiles_derived_class_super_constructor_with_builtin_object() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("derived-class-super-constructor-builtin-object.js");
    let output = tempdir
        .path()
        .join("derived-class-super-constructor-builtin-object.wasm");

    fs::write(
        &input,
        r#"
        class C extends Object {
          constructor() {
            'use strict';
            super();
          }
        }

        let c = new C();
        console.log(typeof c, Object.getPrototypeOf(c) === C.prototype);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "object true\n");
}

fn compiles_class_accessor_computed_yield_name_through_done_loop() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-accessor-computed-yield-name-done-loop.js");
    let output = tempdir
        .path()
        .join("class-accessor-computed-yield-name-done-loop.wasm");

    fs::write(
        &input,
        r#"
        var inst = "unset";
        var stat = "unset";

        function * g() {
          class C {
            get [yield 9]() {
              return 9;
            }

            static get [yield 9]() {
              return 9;
            }
          }

          let c = new C();
          inst = c[yield 9];
          stat = C[yield 9];
        }

        let iter = g();
        while (iter.next().done === false) {}
        console.log(inst, stat);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "9 9\n");
}

#[test]
fn compiles_class_expression_computed_field_name_from_additive_expression() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-expression-computed-field-name-additive.js");
    let output = tempdir
        .path()
        .join("class-expression-computed-field-name-additive.wasm");

    fs::write(
        &input,
        r#"
        let C = class {
          [1 + 1] = 2;
          static [1 + 1] = 2;
        };

        let c = new C();
        console.log(c[2], C[2]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "2 2\n");
}

#[test]
fn compiles_named_class_expression_typeof_and_name_without_self_alias_recursion() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("named-class-expression-typeof-name.js");
    let output = tempdir
        .path()
        .join("named-class-expression-typeof-name.wasm");

    fs::write(
        &input,
        r#"
        var C = class C {};
        console.log(typeof C, C.name);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "function C\n");
}

#[test]
fn compiles_named_class_expression_function_prototype_identity() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-expr-prototype.js");
    let output = tempdir.path().join("class-expr-prototype.wasm");

    fs::write(
        &input,
        r#"
        var C = class C {};
        console.log(
          typeof Object.getPrototypeOf(C),
          Object.getPrototypeOf(C) === Function.prototype
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "object true\n");
}

#[test]
fn compiles_class_extends_side_effects_constructor_prototype_wiring() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-extends-side-effects.js");
    let output = tempdir.path().join("class-extends-side-effects.wasm");

    fs::write(
        &input,
        r#"
        var calls = 0;
        class C {}
        class D extends (calls++, C) {}
        console.log(
          calls,
          Object.getPrototypeOf(D) === C,
          Object.getPrototypeOf(D.prototype) === C.prototype
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1 true true\n");
}

#[test]
fn compiles_test262_assert_samevalue_for_class_extends_side_effects() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("test262-class-extends-side-effects-samevalue.js");
    let output = tempdir
        .path()
        .join("test262-class-extends-side-effects-samevalue.wasm");

    fs::write(
        &input,
        r#"
        function Test262Error(message) {
          this.name = "Test262Error";
          this.message = message ?? "";
        }

        function __sameValue(left, right) {
          if (left === right) {
            return left !== 0 || 1 / left === 1 / right;
          }
          return left !== left && right !== right;
        }

        function __assertSameValue(actual, expected, message) {
          if (__sameValue(actual, expected)) {
            return;
          }
          throw new Test262Error(message ?? "sameValue");
        }

        function $DONE(error) {
          if (error !== undefined) {
            throw error;
          }
        }

        var calls = 0;
        class C {}
        class D extends (calls++, C) {}
        __assertSameValue(calls, 1, "calls");
        __assertSameValue(typeof D, "function", "typeof");
        __assertSameValue(Object.getPrototypeOf(D), C, "ctor proto");
        __assertSameValue(
          C.prototype,
          Object.getPrototypeOf(D.prototype),
          "instance proto"
        );
        console.log("ok");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_test262_assert_samevalue_for_caught_second_super_side_effects() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-second-super-assert-samevalue.js");
    let output = tempdir
        .path()
        .join("class-second-super-assert-samevalue.wasm");

    fs::write(
        &input,
        r#"
        function __assertSameValue(actual, expected, message) {
          if (actual !== expected) {
            throw new Error(message || "sameValue");
          }
        }

        class Base {
          constructor(a, b) {
            var o = new Object();
            o.prp = a + b;
            return o;
          }
        }

        class Subclass2 extends Base {
          constructor(x) {
            super(1, 2);
            if (x < 0) return;
            var called = false;
            function tmp() {
              called = true;
              return 3;
            }
            var exn = null;
            try {
              super(tmp(), 4);
            } catch (e) {
              exn = e;
            }
            __assertSameValue(exn instanceof ReferenceError, true, "exn");
            __assertSameValue(called, true, "called");
          }
        }

        var s2 = new Subclass2(1);
        __assertSameValue(s2.prp, 3, "s2.prp");
        var s3 = new Subclass2(-1);
        __assertSameValue(s3.prp, 3, "s3.prp");
        console.log("ok");
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "ok\n");
}

#[test]
fn compiles_class_expression_computed_field_name_from_logical_and_assignment() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-expression-computed-field-name-logical-and.js");
    let output = tempdir
        .path()
        .join("class-expression-computed-field-name-logical-and.wasm");

    fs::write(
        &input,
        r#"
        let x = 0;
        let C = class {
          [x &&= 1] = 2;
          static [x &&= 1] = 2;
        };

        let c = new C();
        console.log(x, c[0], C[0], c[String(0)], C[String(0)]);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "0 2 2 2 2\n");
}

#[test]
fn compiles_class_expression_computed_field_name_from_identifier() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-expression-computed-field-name-identifier.js");
    let output = tempdir
        .path()
        .join("class-expression-computed-field-name-identifier.wasm");

    fs::write(
        &input,
        r#"
        let x = 1;
        let C = class {
          [x] = '2';
          static [x] = '2';
        };

        let c = new C();
        console.log(
          typeof c[x], c[x],
          typeof C[x], C[x],
          typeof c[String(x)], c[String(x)],
          typeof C[String(x)], C[String(x)]
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "string 2 string 2 string 2 string 2\n"
    );
}

#[test]
fn compiles_class_expression_computed_field_name_from_yield_expression() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("class-expression-computed-field-name-yield.js");
    let output = tempdir
        .path()
        .join("class-expression-computed-field-name-yield.wasm");

    fs::write(
        &input,
        r#"
        let r1;
        let r2;
        let r3;
        let r4;

        function * g() {
          let C = class {
            [yield 9] = 9;
            static [yield 9] = 9;
          };

          let c = new C();
          r1 = c[yield 9];
          r2 = C[yield 9];
          r3 = c[String(yield 9)];
          r4 = C[String(yield 9)];
        }

        let iter = g();
        while (iter.next().done === false) {}
        console.log(r1, r2, r3, r4);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "9 9 9 9\n");
}

#[test]
fn compiles_class_function_name_and_length_precedence() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("class-function-name-and-length.js");
    let output = tempdir.path().join("class-function-name-and-length.wasm");

    fs::write(
        &input,
        r#"
        var namedSym = Symbol('test262');
        var anonSym = Symbol();
        var isDefined = false;

        class A {
          get id() {}
          get [anonSym]() {}
          get [namedSym]() {}
          set id(_) {}
          *gen() {}
          static get length() {
            if (isDefined) return 'pass';
            throw new Error('getter executed during definition');
          }
          static *name() {}
        }

        isDefined = true;

        var getter = Object.getOwnPropertyDescriptor(A.prototype, 'id').get;
        var anonGetter = Object.getOwnPropertyDescriptor(A.prototype, anonSym).get;
        var namedGetter = Object.getOwnPropertyDescriptor(A.prototype, namedSym).get;
        var setter = Object.getOwnPropertyDescriptor(A.prototype, 'id').set;

        console.log(
          A.length,
          typeof A.name,
          getter.name,
          JSON.stringify(anonGetter.name),
          namedGetter.name,
          setter.name,
          A.prototype.gen.name
        );
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "pass function get id \"get \" get [test262] set id gen\n"
    );
}

#[test]
fn compiles_json_stringify_string_primitive() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("json-stringify-string.js");
    let output = tempdir.path().join("json-stringify-string.wasm");

    fs::write(
        &input,
        r#"
        console.log(JSON.stringify("x"), JSON.stringify(undefined), JSON.stringify(null));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "\"x\" undefined null\n"
    );
}

#[test]
fn compiles_bound_function_prototype_call_helpers() {
    let tempdir = tempdir().unwrap();
    let input = tempdir
        .path()
        .join("bound-function-prototype-call-helpers.js");
    let output = tempdir
        .path()
        .join("bound-function-prototype-call-helpers.wasm");

    fs::write(
        &input,
        r#"
        var __join = Function.prototype.call.bind(Array.prototype.join);
        var __push = Function.prototype.call.bind(Array.prototype.push);
        var __hasOwnProperty = Function.prototype.call.bind(Object.prototype.hasOwnProperty);
        var __propertyIsEnumerable = Function.prototype.call.bind(Object.prototype.propertyIsEnumerable);

        var failures = [];
        console.log(__push(failures, "x"), failures.length, failures[0]);
        console.log(__join(["a", "b"], ","), __join(failures, "; "));

        var obj = { x: 1 };
        console.log(__hasOwnProperty(obj, "x"), __hasOwnProperty(obj, "y"));
        console.log(__propertyIsEnumerable(obj, "x"), __propertyIsEnumerable(obj, "toString"));
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "1 1 x\na,b x\ntrue false\ntrue false\n"
    );
}

#[test]
fn compiles_symbol_parameter_descriptor_reads_through_inlined_call_frame() {
    let tempdir = tempdir().unwrap();
    let input = tempdir.path().join("symbol-parameter-descriptor-reads.js");
    let output = tempdir
        .path()
        .join("symbol-parameter-descriptor-reads.wasm");

    fs::write(
        &input,
        r#"
        var anonSym = Symbol();
        var namedSym = Symbol('test262');
        class A {
          get [anonSym]() {}
          get [namedSym]() {}
        }

        function probe(target, key) {
          var getter = Object.getOwnPropertyDescriptor(target, key).get;
          console.log(typeof getter, getter.name);
        }

        probe(A.prototype, anonSym);
        probe(A.prototype, namedSym);
        "#,
    )
    .unwrap();

    let compile = Command::new(env!("CARGO_BIN_EXE_ayeyaiyai"))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert!(
        compile.status.success(),
        "compiler failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr),
    );

    let run = Command::new("wasmtime").arg(&output).output().unwrap();

    assert!(
        run.status.success(),
        "wasmtime failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr),
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "function get \nfunction get [test262]\n"
    );
}
