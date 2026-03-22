# AyeYaiYai

`AyeYaiYai` is a Rust compiler that translates a growing subset of JavaScript directly
into a Preview 2 `.wasm`.

The repository now contains a real end-to-end baseline:

- Parse JavaScript with SWC.
- Lower supported syntax into an internal representation.
- Emit final WebAssembly bytes.
- Run the output with `wasmtime`.

## Status

This is not close to full JavaScript yet, and it is nowhere near `test262` compliance. The current
compiler is intentionally narrow, but it is real and executable.

Supported today:

- `let`, `const`, and `var` declarations with identifier bindings
- assignment, compound assignment (`+=`, `-=`, `*=`, `/=`, `%=`, `&&=`, `||=`, `??=`), and `++` / `--`
- numeric, string, boolean, `null`, and `undefined`
- array literals, object literals, indexed access, property access, and member assignment on array/object bindings
- unary `-` and `!`
- binary `+`, `-`, `*`, `/`, `%`, `===`, `!==`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `??`
- ternary expressions
- top-level `function` declarations, parameter passing, calls, and `return`
- `if` / `else`
- `while` and `for`
- `break` and `continue`
- `console.log(...)`

Not supported yet:

- function expressions, arrow functions, closures, and captured outer variables
- nested/destructuring patterns
- general method calls and `this` binding
- coercive equality
- imports / exports
- exceptions
- the overwhelming majority of ECMAScript semantics required for `test262`

## Usage

Build the compiler:

```bash
cargo build
```

Compile JavaScript to WASI Preview 2:

```bash
cargo run -- examples/sum.js -o sum.wasm
wasmtime sum.wasm
```

```bash
cargo run -- examples/control-flow.js -o control-flow.wasm
wasmtime control-flow.wasm
```

```bash
cargo run -- examples/objects.js -o objects.wasm
wasmtime objects.wasm
```

Inspect direct backend output bytes or run the resulting module:

```bash
cargo run -- examples/sum.js -o sum.wasm
hexdump -C sum.wasm
```

## Example

```javascript
function sumTo(limit) {
  let total = 0;

  for (let i = 0; i <= limit; i++) {
    if (i === 2) {
      continue;
    }

    if (i === 5) {
      break;
    }

    total += i;
  }

  return total;
}

let label = false ? "bad" : "good";
console.log(label, sumTo(10), undefined ?? "fallback");
```

## Development

The current tests include an end-to-end integration path that compiles JavaScript, emits a Preview 2
WebAssembly component, and executes it with `wasmtime`:

```bash
cargo test
```

Run the full `test262` sweep from the repository root with:

```bash
./test262.sh
```

You can also target high-level `test262` categories directly:

```bash
./test262.sh --category language
./test262.sh --category built-ins
./test262.sh --category intl402
```

The wrapper will:

- use `TEST262_DIR` if you set it
- use `TEST262_TIMEOUT_SECONDS` when you want to override the per-test Wasmtime timeout, which defaults to `5`
- otherwise reuse `./test262` or `/tmp/test262` when present
- otherwise clone `https://github.com/tc39/test262.git` into `./.cache/test262`
- accept repeatable `--category` flags for `language`, `built-ins` / `builtins`, `intl402`, `annexB` / `annex-b`, `staging`, and `implementation-contributed`
- still accept raw runner filters like `--contains 'test/language/statements/'` when you want a narrower subtree

At the end of the run it prints a compliance line based on discovered tests:

```text
Compliance: <passed%> (<passed> / <discovered> discovered tests passed)
Attempt rate: <attempted%> (<attempted> / <discovered> discovered tests executed)
```

The full runner log is written to `./.artifacts/test262/latest.log`.

Useful examples:

```bash
# Full corpus
./test262.sh

# Run a specific top-level category
./test262.sh --category language

# Run multiple categories in one sweep
./test262.sh --category language --category built-ins

# Narrow to a subtree while keeping the same reporting format
./test262.sh --contains 'test/language/statements/'

# Run a smaller sample
./test262.sh --category language --contains 'test/language/statements/with/' --limit 40

# Show the available wrapper categories
./test262.sh --list-categories
```
