use std::{fs, process::Command};

use ayeyaiyai::{CompileOptions, compile_file, compile_file_with_goal, compile_source_with_goal};

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
fn compiles_module_goal_sources_on_the_direct_wasm_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let output = tempdir.path().join("module.wasm");
    let options = CompileOptions {
        output: output.clone(),
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
    .unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "1\n");
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
fn compile_file_executes_for_of_over_custom_iterator_breaks_and_closes() {
    let tempdir = tempfile::tempdir().unwrap();
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
    assert_eq!(String::from_utf8_lossy(&run.stdout), "2 1\n");
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
fn compiles_module_goal_files_with_real_paths() {
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
        output: output.clone(),
        target: "wasm32-wasip2".to_string(),
    };

    compile_file_with_goal(&input, &options, true).unwrap();

    let run = Command::new("wasmtime").arg(&output).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");
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
