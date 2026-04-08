use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use ayeyaiyai::{CompileOptions, compile_file, compile_file_with_goal};
use clap::{ArgAction, Parser};
use tempfile::tempdir;
use walkdir::WalkDir;

const ASSERT_PRELUDE: &str = r#"
function Test262Error(message) {
  this.name = "Test262Error";
  this.message = message ?? "";
}

function __formatIdentityFreeValue(value) {
  switch (value === null ? "null" : typeof value) {
    case "string":
      return typeof JSON !== "undefined" ? JSON.stringify(value) : "\"" + value + "\"";
    case "bigint":
      return String(value) + "n";
    case "number":
      if (value === 0 && 1 / value === -Infinity) {
        return "-0";
      }
      return String(value);
    case "boolean":
    case "undefined":
    case "null":
      return String(value);
  }
}

function __sameValue(left, right) {
  if (left === right) {
    return left !== 0 || 1 / left === 1 / right;
  }
  return left !== left && right !== right;
}

function __assertToString(value) {
  var basic = __formatIdentityFreeValue(value);
  if (basic) {
    return basic;
  }
  try {
    return String(value);
  } catch (error) {
    if (error && error.name === "TypeError") {
      return Object.prototype.toString.call(value);
    }
    throw error;
  }
}

function assert(mustBeTrue, message) {
  if (mustBeTrue === true) {
    return;
  }
  if (message === undefined) {
    message = "Expected true but got " + __assertToString(mustBeTrue);
  }
  throw new Test262Error(message);
}

globalThis.assert = assert;

function __assert(condition, message) {
  assert(condition, message);
}

function __assertSameValue(actual, expected, message) {
  try {
    if (__sameValue(actual, expected)) {
      return;
    }
  } catch (error) {
    throw new Test262Error((message ?? "") + " (_isSameValue operation threw) " + error);
  }

  if (message === undefined) {
    message = "";
  } else {
    message += " ";
  }

  message += "Expected SameValue(«" + __assertToString(actual) + "», «" + __assertToString(expected) + "») to be true";
  throw new Test262Error(message);
}

function __assertNotSameValue(actual, expected, message) {
  if (!__sameValue(actual, expected)) {
    return;
  }

  if (message === undefined) {
    message = "";
  } else {
    message += " ";
  }

  message += "Expected SameValue(«" + __assertToString(actual) + "», «" + __assertToString(expected) + "») to be false";
  throw new Test262Error(message);
}

function __ayyAssertThrows(expectedErrorConstructor, func, message) {
  var expectedName, actualName;

  if (typeof func !== "function") {
    throw new Test262Error("assert.throws requires two arguments: the error constructor and a function to run");
  }

  if (message === undefined) {
    message = "";
  } else {
    message += " ";
  }

  try {
    func();
  } catch (thrown) {
    if (typeof thrown !== "object" || thrown === null) {
      throw new Test262Error(message + "Thrown value was not an object!");
    } else if (thrown.constructor !== expectedErrorConstructor) {
      expectedName = expectedErrorConstructor.name;
      actualName = thrown.constructor.name;
      if (expectedName === actualName) {
        message += "Expected a " + expectedName + " but got a different error constructor with the same name";
      } else {
        message += "Expected a " + expectedName + " but got a " + actualName;
      }
      throw new Test262Error(message);
    }
    return;
  }

  throw new Test262Error(message + "Expected a " + expectedErrorConstructor.name + " to be thrown but no exception was thrown at all");
}

assert._isSameValue = __sameValue;
assert._toString = __assertToString;
assert.sameValue = __assertSameValue;
assert.notSameValue = __assertNotSameValue;
assert.throws = __ayyAssertThrows;

function compareArray(actual, expected) {
  if (actual.length !== expected.length) {
    return false;
  }
  for (var i = 0; i < actual.length; i += 1) {
    if (!__sameValue(actual[i], expected[i])) {
      return false;
    }
  }
  return true;
}

compareArray.format = function (arrayLike) {
  return "" + arrayLike;
};

assert.compareArray = function (actual, expected, message) {
  if (!compareArray(actual, expected)) {
    throw new Test262Error(message ?? "compareArray");
  }
};
"#;

const FN_GLOBAL_OBJECT_PRELUDE: &str = r#"
function fnGlobalObject() {
  return globalThis;
}
"#;

const DONE_PRELUDE: &str = r#"
function $DONE(error) {
  if (error !== undefined) {
    throw error;
  }
}
"#;

const ASYNC_HELPERS_PRELUDE: &str = r#"
function asyncTest(testFunc) {
  if (typeof testFunc !== "function") {
    $DONE(new Test262Error("asyncTest called with non-function argument"));
    return;
  }
  try {
    testFunc().then(
      function () {
        $DONE();
      },
      function (error) {
        $DONE(error);
      }
    );
  } catch (syncError) {
    $DONE(syncError);
  }
}

assert.throwsAsync = function (expectedErrorConstructor, func, message) {
  return new Promise(function (resolve, reject) {
    var fail = function (detail) {
      reject(new Test262Error(message === undefined ? detail : message + " " + detail));
    };
    if (typeof expectedErrorConstructor !== "function") {
      fail("assert.throwsAsync called with an argument that is not an error constructor");
      return;
    }
    if (typeof func !== "function") {
      fail("assert.throwsAsync called with an argument that is not a function");
      return;
    }
    var expectedName = expectedErrorConstructor.name;
    var expectation = "Expected a " + expectedName + " to be thrown asynchronously";
    var result;
    try {
      result = func();
    } catch (thrown) {
      fail(expectation + " but the function threw synchronously");
      return;
    }
    if (result === null || typeof result !== "object" || typeof result.then !== "function") {
      fail(expectation + " but result was not a thenable");
      return;
    }
    result.then(
      function () {
        fail(expectation + " but no exception was thrown at all");
      },
      function (thrown) {
        if (thrown === null || typeof thrown !== "object") {
          fail(expectation + " but thrown value was not an object");
          return;
        }
        if (thrown.constructor !== expectedErrorConstructor) {
          var actualName = thrown.constructor && thrown.constructor.name || typeof thrown;
          fail(expectation + " but got a " + actualName);
          return;
        }
        resolve();
      }
    );
  });
};
"#;

const RESIZABLE_ARRAYBUFFER_UTILS_PRELUDE: &str = r#"
var MyUint8Array;
var MyFloat32Array;
var MyBigInt64Array;

try {
  MyUint8Array = class MyUint8Array extends Uint8Array {};
} catch (e) {}

try {
  MyFloat32Array = class MyFloat32Array extends Float32Array {};
} catch (e) {}

try {
  MyBigInt64Array = class MyBigInt64Array extends BigInt64Array {};
} catch (e) {}

const builtinCtors = [
  Uint8Array,
  Int8Array,
  Uint16Array,
  Int16Array,
  Uint32Array,
  Int32Array,
  Float32Array,
  Float64Array,
  Uint8ClampedArray,
];

if (typeof Float16Array !== "undefined") {
  builtinCtors.push(Float16Array);
}

if (typeof BigUint64Array !== "undefined") {
  builtinCtors.push(BigUint64Array);
}

if (typeof BigInt64Array !== "undefined") {
  builtinCtors.push(BigInt64Array);
}

const floatCtors = [Float32Array, Float64Array];
if (typeof MyFloat32Array !== "undefined") {
  floatCtors.push(MyFloat32Array);
}
if (typeof Float16Array !== "undefined") {
  floatCtors.push(Float16Array);
}

const ctors = [
  Uint8Array,
  Int8Array,
  Uint16Array,
  Int16Array,
  Uint32Array,
  Int32Array,
  Float32Array,
  Float64Array,
  Uint8ClampedArray,
];
if (typeof Float16Array !== "undefined") {
  ctors.push(Float16Array);
}
if (typeof BigUint64Array !== "undefined") {
  ctors.push(BigUint64Array);
}
if (typeof BigInt64Array !== "undefined") {
  ctors.push(BigInt64Array);
}
if (typeof MyUint8Array !== "undefined") {
  ctors.push(MyUint8Array);
}
if (typeof MyFloat32Array !== "undefined") {
  ctors.push(MyFloat32Array);
}
if (typeof MyBigInt64Array !== "undefined") {
  ctors.push(MyBigInt64Array);
}

function CreateResizableArrayBuffer(byteLength, maxByteLength) {
  return new ArrayBuffer(byteLength, { maxByteLength: maxByteLength });
}

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
  assert.sameValue(typeof n, "number");
  if ((typeof BigInt64Array !== "undefined" && ta instanceof BigInt64Array) ||
      (typeof BigUint64Array !== "undefined" && ta instanceof BigUint64Array)) {
    return BigInt(n);
  }
  return n;
}
"#;

const DECIMAL_TO_HEX_STRING_PRELUDE: &str = r#"
function decimalToHexString(n) {
  var hex = "0123456789ABCDEF";
  n >>>= 0;
  var s = "";
  while (n) {
    s = hex[n & 0xf] + s;
    n >>>= 4;
  }
  while (s.length < 4) {
    s = "0" + s;
  }
  return s;
}

function decimalToPercentHexString(n) {
  var hex = "0123456789ABCDEF";
  return "%" + hex[(n >> 4) & 0xf] + hex[n & 0xf];
}
"#;

#[derive(Debug, Parser)]
#[command(about = "Run a supported subset of test262 against AyeYaiYai")]
struct Cli {
    #[arg(long)]
    test262_dir: PathBuf,

    #[arg(long, default_value = "wasm32-wasip2")]
    target: String,

    #[arg(long = "test", action = ArgAction::Append)]
    tests: Vec<String>,

    #[arg(long, action = ArgAction::Append)]
    contains: Vec<String>,

    #[arg(long)]
    limit: Option<usize>,

    #[arg(long, default_value_t = 5)]
    timeout_seconds: u64,
}

#[derive(Debug, Default)]
struct Summary {
    discovered: usize,
    attempted: usize,
    passed: usize,
    compile_failed: usize,
    runtime_failed: usize,
    skipped_metadata: usize,
    skipped_content: usize,
}

#[derive(Debug, Default)]
struct Metadata {
    includes: Vec<String>,
    flags: Vec<String>,
    negative: Option<NegativeExpectation>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct NegativeExpectation {
    phase: Option<String>,
    error_type: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut summary = Summary::default();
    let exact_tests = normalize_requested_tests(&cli.test262_dir, &cli.tests)?;

    for entry in WalkDir::new(cli.test262_dir.join("test"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension() == Some(OsStr::new("js")))
    {
        if cli.limit.is_some_and(|limit| summary.attempted >= limit) {
            break;
        }

        let path = entry.path();
        let display = path.display().to_string();
        let relative_display = path
            .strip_prefix(&cli.test262_dir)
            .map(normalize_path_display)
            .unwrap_or_else(|_| normalize_path_display(path));

        if !exact_tests.is_empty() && !exact_tests.contains(&relative_display) {
            continue;
        }

        if !cli.contains.is_empty()
            && !cli
                .contains
                .iter()
                .any(|contains| relative_display.contains(contains))
        {
            continue;
        }

        if should_skip_path(path) {
            continue;
        }

        summary.discovered += 1;

        let source =
            fs::read_to_string(path).with_context(|| format!("failed to read `{display}`"))?;
        let (metadata, body) = parse_test262_source(&source);

        if should_skip_metadata(&metadata) {
            summary.skipped_metadata += 1;
            continue;
        }

        let Some(rewritten) = prepare_test_source(&metadata, &body, &cli.test262_dir) else {
            summary.skipped_content += 1;
            continue;
        };

        if std::env::var_os("AYY_DUMP_PREPARED_SOURCE").is_some() {
            println!("{rewritten}");
            return Ok(());
        }

        summary.attempted += 1;

        let is_module = metadata.flags.iter().any(|flag| flag == "module");

        let outcome = run_single_test(
            path,
            &rewritten,
            &cli.target,
            cli.timeout_seconds,
            is_module,
        );

        match apply_negative_expectation(&metadata, outcome) {
            Ok(()) => {
                summary.passed += 1;
                println!("PASS {display}");
            }
            Err(TestFailure::Compile(error)) => {
                summary.compile_failed += 1;
                println!("COMPILE_FAIL {display}\n{error}");
            }
            Err(TestFailure::Runtime(error)) => {
                summary.runtime_failed += 1;
                println!("RUNTIME_FAIL {display}\n{error}");
            }
        }
    }

    let compliance_percent = if summary.discovered == 0 {
        0.0
    } else {
        (summary.passed as f64 / summary.discovered as f64) * 100.0
    };
    let attempt_rate_percent = if summary.discovered == 0 {
        0.0
    } else {
        (summary.attempted as f64 / summary.discovered as f64) * 100.0
    };

    println!(
        "SUMMARY discovered={} attempted={} passed={} compile_failed={} runtime_failed={} skipped_metadata={} skipped_content={} attempt_rate_percent={:.2} compliance_percent={:.2}",
        summary.discovered,
        summary.attempted,
        summary.passed,
        summary.compile_failed,
        summary.runtime_failed,
        summary.skipped_metadata,
        summary.skipped_content,
        attempt_rate_percent,
        compliance_percent,
    );

    Ok(())
}

fn normalize_requested_tests(test262_dir: &Path, tests: &[String]) -> Result<HashSet<String>> {
    tests
        .iter()
        .map(|test| normalize_requested_test(test262_dir, test))
        .collect()
}

fn normalize_requested_test(test262_dir: &Path, test: &str) -> Result<String> {
    let requested = Path::new(test);
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else if requested
        .components()
        .next()
        .is_some_and(|component| component.as_os_str() == "test")
    {
        test262_dir.join(requested)
    } else {
        test262_dir.join("test").join(requested)
    };

    if !candidate.is_file() {
        anyhow::bail!(
            "exact test `{test}` was not found under `{}`",
            test262_dir.display()
        );
    }

    let relative = candidate.strip_prefix(test262_dir).with_context(|| {
        format!(
            "exact test `{}` must live under `{}`",
            candidate.display(),
            test262_dir.display()
        )
    })?;

    Ok(normalize_path_display(relative))
}

fn normalize_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

enum TestFailure {
    Compile(String),
    Runtime(String),
}

fn run_single_test(
    source_path: &Path,
    source: &str,
    target: &str,
    timeout_seconds: u64,
    module: bool,
) -> Result<(), TestFailure> {
    let tempdir = tempdir().map_err(|error| TestFailure::Compile(error.to_string()))?;
    let source_root = tempdir.path().join("source");
    fs::create_dir_all(&source_root).map_err(|error| TestFailure::Compile(error.to_string()))?;
    let entry_name = source_path
        .file_name()
        .unwrap_or_else(|| OsStr::new("test.js"));
    let entry_path = source_root.join(entry_name);

    if module || source.contains("import(") {
        stage_module_siblings(source_path, &source_root)
            .map_err(|error| TestFailure::Compile(error.to_string()))?;
    }
    fs::write(&entry_path, source).map_err(|error| TestFailure::Compile(error.to_string()))?;

    let wasm_path = tempdir.path().join("test.wasm");
    let options = CompileOptions {
        output: wasm_path.clone(),
        target: target.to_string(),
    };

    let compile_result = if module {
        compile_file_with_goal(&entry_path, &options, true)
    } else {
        compile_file(&entry_path, &options)
    };

    compile_result.map_err(|error| TestFailure::Compile(format!("{error:#}")))?;

    if std::env::var_os("AYY_COMPILE_ONLY").is_some() {
        return Ok(());
    }

    let mut child = Command::new("wasmtime")
        .arg(&wasm_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| TestFailure::Runtime(error.to_string()))?;

    let timeout = Duration::from_secs(timeout_seconds);
    let started = Instant::now();

    loop {
        if child
            .try_wait()
            .map_err(|error| TestFailure::Runtime(error.to_string()))?
            .is_some()
        {
            break;
        }

        if started.elapsed() >= timeout {
            child
                .kill()
                .map_err(|error| TestFailure::Runtime(error.to_string()))?;
            let output = child
                .wait_with_output()
                .map_err(|error| TestFailure::Runtime(error.to_string()))?;
            return Err(TestFailure::Runtime(format!(
                "timed out after {}s\nstdout:\n{}\nstderr:\n{}",
                timeout_seconds,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            )));
        }

        thread::sleep(Duration::from_millis(10));
    }

    let output = child
        .wait_with_output()
        .map_err(|error| TestFailure::Runtime(error.to_string()))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(TestFailure::Runtime(format!(
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )))
    }
}

fn stage_module_siblings(source_path: &Path, target_dir: &Path) -> Result<()> {
    let Some(parent) = source_path.parent() else {
        return Ok(());
    };

    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let path = entry.path();
        if path == source_path || path.extension() != Some(OsStr::new("js")) {
            continue;
        }

        let target = target_dir.join(entry.file_name());
        fs::copy(&path, &target).with_context(|| format!("failed to copy `{}`", path.display()))?;
    }

    Ok(())
}

fn parse_test262_source(source: &str) -> (Metadata, String) {
    let Some(start) = source.find("/*---") else {
        return (Metadata::default(), source.to_string());
    };
    let prefix = &source[..start];
    let rest = &source[start + "/*---".len()..];
    let Some((frontmatter, body)) = rest.split_once("---*/") else {
        return (Metadata::default(), source.to_string());
    };

    let metadata = parse_frontmatter(frontmatter);
    (metadata, format!("{prefix}{}", body.trim_start()))
}

fn parse_frontmatter(frontmatter: &str) -> Metadata {
    let mut metadata = Metadata::default();
    let mut active_list: Option<&str> = None;
    let mut in_negative = false;

    for raw_line in frontmatter.lines() {
        let line = raw_line.trim_end();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if let Some(item) = trimmed.strip_prefix("- ") {
            match active_list {
                Some("flags") => metadata.flags.push(item.trim().to_string()),
                Some("includes") => metadata.includes.push(item.trim().to_string()),
                _ => {}
            }
            continue;
        }

        active_list = None;
        if !raw_line.starts_with(' ') && !raw_line.starts_with('\t') {
            in_negative = false;
        }

        if trimmed.starts_with("negative:") {
            metadata.negative = Some(NegativeExpectation::default());
            in_negative = true;
        } else if in_negative {
            if let Some(phase) = trimmed.strip_prefix("phase:") {
                metadata.negative.get_or_insert_with(Default::default).phase =
                    Some(phase.trim().to_string());
                continue;
            }
            if let Some(error_type) = trimmed.strip_prefix("type:") {
                metadata
                    .negative
                    .get_or_insert_with(Default::default)
                    .error_type = Some(error_type.trim().to_string());
                continue;
            }
        } else if let Some(values) = parse_inline_list(trimmed, "flags:") {
            metadata.flags.extend(values);
        } else if trimmed == "flags:" {
            active_list = Some("flags");
        } else if let Some(values) = parse_inline_list(trimmed, "includes:") {
            metadata.includes.extend(values);
        } else if trimmed == "includes:" {
            active_list = Some("includes");
        }
    }

    metadata
}

fn parse_inline_list(line: &str, key: &str) -> Option<Vec<String>> {
    let remainder = line.strip_prefix(key)?.trim();
    let inner = remainder.strip_prefix('[')?.strip_suffix(']')?;

    if inner.trim().is_empty() {
        return Some(Vec::new());
    }

    Some(
        inner
            .split(',')
            .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
            .collect(),
    )
}

fn should_skip_metadata(metadata: &Metadata) -> bool {
    let _ = metadata;
    false
}

fn prepare_test_source(metadata: &Metadata, body: &str, test262_dir: &Path) -> Option<String> {
    if metadata.flags.iter().any(|flag| flag == "raw")
        && (body.starts_with("#!") || body.starts_with("\u{FEFF}#!"))
    {
        return Some(body.to_string());
    }

    match metadata
        .negative
        .as_ref()
        .and_then(|negative| negative.phase.as_deref())
    {
        Some("parse" | "resolution") => {
            let strict_prefix = metadata
                .flags
                .iter()
                .any(|flag| flag == "onlyStrict")
                .then_some("\"use strict\";\n")
                .unwrap_or_default();
            Some(format!("{strict_prefix}{body}"))
        }
        _ => rewrite_for_supported_subset(metadata, body, &test262_dir.join("harness")),
    }
}

fn apply_negative_expectation(
    metadata: &Metadata,
    outcome: Result<(), TestFailure>,
) -> Result<(), TestFailure> {
    let Some(negative) = metadata.negative.as_ref() else {
        return outcome;
    };

    match negative.phase.as_deref() {
        Some("parse" | "resolution") => match outcome {
            Err(TestFailure::Compile(_)) => Ok(()),
            Ok(()) => Err(TestFailure::Runtime(format!(
                "expected {} failure, but test succeeded",
                negative.phase.as_deref().unwrap_or("negative")
            ))),
            Err(TestFailure::Runtime(error)) => Err(TestFailure::Runtime(format!(
                "expected compile-time {} failure, but execution failed at runtime:\n{error}",
                negative.phase.as_deref().unwrap_or("negative")
            ))),
        },
        Some("runtime") => match outcome {
            Err(TestFailure::Runtime(error))
                if negative
                    .error_type
                    .as_deref()
                    .is_none_or(|expected| error.contains(expected)) =>
            {
                Ok(())
            }
            Err(TestFailure::Runtime(error)) => Err(TestFailure::Runtime(format!(
                "runtime failure did not match expected {:?}:\n{error}",
                negative.error_type
            ))),
            Ok(()) => Err(TestFailure::Runtime(format!(
                "expected runtime {:?} failure, but test succeeded",
                negative.error_type
            ))),
            Err(TestFailure::Compile(error)) => Err(TestFailure::Compile(format!(
                "expected runtime {:?} failure, but compilation failed:\n{error}",
                negative.error_type
            ))),
        },
        _ => outcome,
    }
}

fn include_prelude(include: &str, harness_dir: &Path) -> Option<String> {
    match include {
        "assert.js" | "sta.js" => Some(String::new()),
        "compareArray.js" => Some(String::new()),
        "fnGlobalObject.js" => Some(FN_GLOBAL_OBJECT_PRELUDE.to_string()),
        "asyncHelpers.js" => Some(ASYNC_HELPERS_PRELUDE.to_string()),
        "decimalToHexString.js" => Some(DECIMAL_TO_HEX_STRING_PRELUDE.to_string()),
        "resizableArrayBufferUtils.js" => Some(RESIZABLE_ARRAYBUFFER_UTILS_PRELUDE.to_string()),
        _ => fs::read_to_string(harness_dir.join(include)).ok(),
    }
}

fn should_skip_path(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_FIXTURE.js"))
}

fn supported_subset_haystack(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Code,
        SingleQuoted,
        DoubleQuoted,
        Template,
        LineComment,
        BlockComment,
    }

    let characters = source.chars().collect::<Vec<_>>();
    let mut haystack = String::with_capacity(source.len());
    let mut state = State::Code;
    let mut index = 0;

    while index < characters.len() {
        let character = characters[index];
        let next = characters.get(index + 1).copied();

        match state {
            State::Code => {
                if character == '\'' {
                    state = State::SingleQuoted;
                    haystack.push(' ');
                    index += 1;
                    continue;
                }
                if character == '"' {
                    state = State::DoubleQuoted;
                    haystack.push(' ');
                    index += 1;
                    continue;
                }
                if character == '`' {
                    state = State::Template;
                    haystack.push(' ');
                    index += 1;
                    continue;
                }
                if character == '/' && next == Some('/') {
                    state = State::LineComment;
                    haystack.push(' ');
                    haystack.push(' ');
                    index += 2;
                    continue;
                }
                if character == '/' && next == Some('*') {
                    state = State::BlockComment;
                    haystack.push(' ');
                    haystack.push(' ');
                    index += 2;
                    continue;
                }
                haystack.push(character);
                index += 1;
            }
            State::SingleQuoted | State::DoubleQuoted | State::Template => {
                if character == '\\' {
                    haystack.push(' ');
                    index += 1;
                    if index < characters.len() {
                        haystack.push(' ');
                        index += 1;
                    }
                    continue;
                }

                let closing = match state {
                    State::SingleQuoted => '\'',
                    State::DoubleQuoted => '"',
                    State::Template => '`',
                    _ => unreachable!(),
                };
                if character == closing {
                    state = State::Code;
                }
                haystack.push(if character == '\n' { '\n' } else { ' ' });
                index += 1;
            }
            State::LineComment => {
                haystack.push(if character == '\n' { '\n' } else { ' ' });
                index += 1;
                if character == '\n' {
                    state = State::Code;
                }
            }
            State::BlockComment => {
                if character == '*' && next == Some('/') {
                    haystack.push(' ');
                    haystack.push(' ');
                    index += 2;
                    state = State::Code;
                    continue;
                }
                haystack.push(if character == '\n' { '\n' } else { ' ' });
                index += 1;
            }
        }
    }

    haystack
}

fn rewrite_for_supported_subset(
    metadata: &Metadata,
    body: &str,
    harness_dir: &Path,
) -> Option<String> {
    let unsupported_markers = ["$DONOTEVALUATE"];
    let searchable = supported_subset_haystack(&body);

    if unsupported_markers
        .iter()
        .any(|marker| searchable.contains(marker))
    {
        return None;
    }

    let include_prelude = metadata
        .includes
        .iter()
        .map(|include| include_prelude(include, harness_dir))
        .collect::<Option<Vec<_>>>()?
        .join("\n");

    let rewritten = body
        .replace("assert.throws(", "__ayyAssertThrows(")
        .replace("assert.sameValue(", "__assertSameValue(")
        .replace("assert.notSameValue(", "__assertNotSameValue(")
        .replace("assert(", "__assert(");

    let strict_prefix = metadata
        .flags
        .iter()
        .any(|flag| flag == "onlyStrict")
        .then_some("\"use strict\";\n")
        .unwrap_or_default();

    Some(format!(
        "{strict_prefix}{ASSERT_PRELUDE}\n{DONE_PRELUDE}\n{include_prelude}\n{rewritten}"
    ))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::{
        Metadata, NegativeExpectation, TestFailure, apply_negative_expectation,
        normalize_requested_test, parse_frontmatter, parse_test262_source, prepare_test_source,
        rewrite_for_supported_subset, should_skip_metadata, should_skip_path,
        supported_subset_haystack,
    };

    #[test]
    fn rewrites_only_strict_tests_with_directive_prefix() {
        let metadata = Metadata {
            flags: vec!["onlyStrict".to_string()],
            ..Metadata::default()
        };

        let rewritten = rewrite_for_supported_subset(
            &metadata,
            "assert.sameValue(1, 1);",
            Path::new(".cache/test262/harness"),
        )
        .unwrap();

        assert!(rewritten.starts_with("\"use strict\";\n"));
    }

    #[test]
    fn exposes_assert_helpers_to_harness_preludes() {
        let rewritten = rewrite_for_supported_subset(
            &Metadata::default(),
            "",
            Path::new(".cache/test262/harness"),
        )
        .unwrap();

        assert!(rewritten.contains("assert.sameValue = __assertSameValue;"));
        assert!(rewritten.contains("assert.notSameValue = __assertNotSameValue;"));
        assert!(rewritten.contains("assert.throws = __ayyAssertThrows;"));
    }

    #[test]
    fn exposes_callable_assert_to_harness_preludes() {
        let rewritten = rewrite_for_supported_subset(
            &Metadata::default(),
            "",
            Path::new(".cache/test262/harness"),
        )
        .unwrap();

        assert!(rewritten.contains("function assert(mustBeTrue, message) {"));
        assert!(rewritten.contains("globalThis.assert = assert;"));
        assert!(rewritten.contains("assert._toString = __assertToString;"));
    }

    #[test]
    fn resizable_arraybuffer_prelude_avoids_constructor_slice_copy() {
        let metadata = Metadata {
            includes: vec!["resizableArrayBufferUtils.js".to_string()],
            ..Metadata::default()
        };

        let rewritten =
            rewrite_for_supported_subset(&metadata, "", Path::new(".cache/test262/harness"))
                .unwrap();

        assert!(!rewritten.contains("builtinCtors.slice()"));
        assert!(rewritten.contains("const ctors = ["));
    }

    #[test]
    fn preserves_raw_leading_hashbang_tests_without_prelude_injection() {
        let metadata = Metadata {
            flags: vec!["raw".to_string()],
            ..Metadata::default()
        };

        let source =
            prepare_test_source(&metadata, "#! comment\r{}\n", Path::new(".cache/test262"))
                .unwrap();

        assert_eq!(source, "#! comment\r{}\n");
    }

    #[test]
    fn preserves_prefix_before_frontmatter_in_test_source() {
        let (metadata, body) = parse_test262_source(
            r#"#\041

/*---
flags: [raw]
negative:
  phase: parse
  type: SyntaxError
---*/

throw "unreachable";
"#,
        );

        assert_eq!(metadata.flags, vec!["raw".to_string()]);
        assert!(
            body.starts_with("#\\041\n\nthrow \"unreachable\";"),
            "{body:?}"
        );
    }

    #[test]
    fn does_not_skip_module_flag_tests() {
        let metadata = Metadata {
            flags: vec!["module".to_string()],
            ..Metadata::default()
        };

        assert!(!should_skip_metadata(&metadata));
    }

    #[test]
    fn does_not_skip_generated_positive_tests() {
        let metadata = Metadata {
            flags: vec!["generated".to_string(), "module".to_string()],
            ..Metadata::default()
        };

        assert!(!should_skip_metadata(&metadata));
    }

    #[test]
    fn does_not_skip_raw_tests() {
        let metadata = Metadata {
            flags: vec!["raw".to_string(), "module".to_string()],
            ..Metadata::default()
        };

        assert!(!should_skip_metadata(&metadata));
    }

    #[test]
    fn does_not_skip_negative_tests() {
        let metadata = Metadata {
            negative: Some(NegativeExpectation::default()),
            ..Metadata::default()
        };

        assert!(!should_skip_metadata(&metadata));
    }

    #[test]
    fn does_not_skip_async_tests() {
        let metadata = Metadata {
            flags: vec!["module".to_string(), "async".to_string()],
            ..Metadata::default()
        };

        assert!(!should_skip_metadata(&metadata));
    }

    #[test]
    fn parses_negative_phase_and_type() {
        let metadata = parse_frontmatter(
            r#"
negative:
  phase: parse
  type: SyntaxError
"#,
        );

        assert_eq!(
            metadata.negative,
            Some(NegativeExpectation {
                phase: Some("parse".to_string()),
                error_type: Some("SyntaxError".to_string()),
            })
        );
    }

    #[test]
    fn prepares_negative_parse_tests_without_assert_prelude() {
        let metadata = Metadata {
            flags: vec!["onlyStrict".to_string()],
            negative: Some(NegativeExpectation {
                phase: Some("parse".to_string()),
                error_type: Some("SyntaxError".to_string()),
            }),
            ..Metadata::default()
        };

        let source =
            prepare_test_source(&metadata, "await 1;", Path::new(".cache/test262")).unwrap();

        assert!(source.starts_with("\"use strict\";\nawait 1;"));
        assert!(!source.contains("function __assert("));
    }

    #[test]
    fn rewrites_property_helper_include() {
        let metadata = Metadata {
            includes: vec!["propertyHelper.js".to_string()],
            ..Metadata::default()
        };

        let source = rewrite_for_supported_subset(
            &metadata,
            "verifyProperty({}, 'x', {});",
            Path::new(".cache/test262/harness"),
        )
        .expect("propertyHelper.js should be supported");

        assert!(source.contains("function verifyProperty("));
    }

    #[test]
    fn parse_negative_tests_pass_on_compile_failure() {
        let metadata = Metadata {
            negative: Some(NegativeExpectation {
                phase: Some("parse".to_string()),
                error_type: Some("SyntaxError".to_string()),
            }),
            ..Metadata::default()
        };

        assert!(
            apply_negative_expectation(&metadata, Err(TestFailure::Compile("syntax".to_string())))
                .is_ok()
        );
    }

    #[test]
    fn runtime_negative_tests_require_matching_error_type() {
        let metadata = Metadata {
            negative: Some(NegativeExpectation {
                phase: Some("runtime".to_string()),
                error_type: Some("TypeError".to_string()),
            }),
            ..Metadata::default()
        };

        assert!(
            apply_negative_expectation(
                &metadata,
                Err(TestFailure::Runtime("TypeError: boom".to_string()))
            )
            .is_ok()
        );
        assert!(
            apply_negative_expectation(
                &metadata,
                Err(TestFailure::Runtime("ReferenceError: boom".to_string()))
            )
            .is_err()
        );
    }

    #[test]
    fn does_not_skip_export_source_rewrites() {
        let metadata = Metadata {
            flags: vec!["module".to_string()],
            ..Metadata::default()
        };

        assert!(
            rewrite_for_supported_subset(
                &metadata,
                "export var value = 1;",
                Path::new(".cache/test262/harness"),
            )
            .is_some()
        );
    }

    #[test]
    fn rewrites_assert_throws_helpers() {
        let metadata = Metadata::default();
        let rewritten = rewrite_for_supported_subset(
            &metadata,
            "assert.throws(TypeError, function() { throw new Test262Error(); });",
            Path::new(".cache/test262/harness"),
        )
        .unwrap();

        assert!(rewritten.contains("__ayyAssertThrows("));
        assert!(rewritten.contains("throw new Test262Error();"));
    }

    #[test]
    fn injects_test262_error_prelude() {
        let metadata = Metadata::default();
        let rewritten = rewrite_for_supported_subset(
            &metadata,
            "new Test262Error();",
            Path::new(".cache/test262/harness"),
        )
        .unwrap();

        assert!(rewritten.contains("function Test262Error("));
    }

    #[test]
    fn supports_fn_global_object_include() {
        let metadata = Metadata {
            includes: vec!["fnGlobalObject.js".to_string()],
            ..Metadata::default()
        };

        let rewritten = rewrite_for_supported_subset(
            &metadata,
            "assert.sameValue(fnGlobalObject(), globalThis);",
            Path::new(".cache/test262/harness"),
        )
        .unwrap();

        assert!(rewritten.contains("function fnGlobalObject()"));
    }

    #[test]
    fn allows_class_and_try_catch_in_supported_subset() {
        let metadata = Metadata {
            flags: vec!["module".to_string()],
            ..Metadata::default()
        };

        let body = r#"
        class Example {}
        try { throw new TypeError(); } catch (error) {}
        "#;

        assert!(
            rewrite_for_supported_subset(&metadata, body, Path::new(".cache/test262/harness"))
                .is_some()
        );
    }

    #[test]
    fn skips_fixture_files() {
        assert!(should_skip_path(Path::new(
            "/tmp/test262/test/language/module-code/eval-rqstd-order-1_FIXTURE.js"
        )));
        assert!(!should_skip_path(Path::new(
            "/tmp/test262/test/language/expressions/yield/rhs-iter.js"
        )));
    }

    #[test]
    fn ignores_comments_when_scanning_for_unsupported_markers() {
        let metadata = Metadata {
            flags: vec!["module".to_string()],
            ..Metadata::default()
        };
        let body = r#"
        // This invocation should not throw an exception
        Reflect.preventExtensions({});
        "#;

        assert!(!supported_subset_haystack(body).contains("throw "));
        assert!(
            rewrite_for_supported_subset(&metadata, body, Path::new(".cache/test262/harness"))
                .is_some()
        );
    }

    #[test]
    fn loads_local_harness_includes_from_test262_directory() {
        let tempdir = tempfile::tempdir().unwrap();
        let harness_dir = tempdir.path().join("harness");
        fs::create_dir_all(&harness_dir).unwrap();
        fs::write(harness_dir.join("customHarness.js"), "var injected = 1;").unwrap();

        let metadata = Metadata {
            includes: vec!["customHarness.js".to_string()],
            ..Metadata::default()
        };

        let rewritten =
            rewrite_for_supported_subset(&metadata, "assert.sameValue(injected, 1);", &harness_dir)
                .unwrap();

        assert!(rewritten.contains("var injected = 1;"));
    }

    #[test]
    fn supports_resizable_arraybuffer_utils_without_function_constructor() {
        let metadata = Metadata {
            includes: vec!["resizableArrayBufferUtils.js".to_string()],
            ..Metadata::default()
        };

        let rewritten = rewrite_for_supported_subset(
            &metadata,
            "assert.sameValue(typeof CreateResizableArrayBuffer, 'function');",
            Path::new(".cache/test262/harness"),
        )
        .unwrap();

        assert!(rewritten.contains("function CreateResizableArrayBuffer("));
        assert!(!rewritten.contains("new Function("));
    }

    #[test]
    fn normalizes_exact_test_paths_from_test_prefix() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir
            .path()
            .join("test/language/expressions/yield/rhs-iter.js");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "").unwrap();

        let normalized = normalize_requested_test(
            tempdir.path(),
            "test/language/expressions/yield/rhs-iter.js",
        )
        .unwrap();

        assert_eq!(normalized, "test/language/expressions/yield/rhs-iter.js");
    }

    #[test]
    fn normalizes_exact_test_paths_from_language_prefix() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir
            .path()
            .join("test/language/expressions/yield/rhs-iter.js");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "").unwrap();

        let normalized =
            normalize_requested_test(tempdir.path(), "language/expressions/yield/rhs-iter.js")
                .unwrap();

        assert_eq!(normalized, "test/language/expressions/yield/rhs-iter.js");
    }
}
