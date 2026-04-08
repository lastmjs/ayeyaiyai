## Goal

Do not stop until you have successfully implemented a compiler (written in the Rust language) called `AyeYaiYai` that compiles JavaScript into WASI 0.2 (Preview 2). The compiler itself must be written in Rust, but it must directly emit the final Wasm/WASI output itself. You should not just interpret JavaScript in Wasi, like with Boa or QuickJS compiled into Wasi. No, you must literally compile JavaScript into WASI 0.2 (Preview 2).

`AyeYaiYai` must perform ahead-of-time compilation from JavaScript source directly to Wasm/WASI. The compiler must create the Wasm bytes directly itself from Rust. There must be no intermediate generated source language or source-like representation that is then compiled or assembled into Wasm, including generated Rust, C, C++, Zig, WAT, or any other language handed off to `rustc`, `clang`, `zig`, `wat2wasm`, or another compiler toolchain stage. The generated module must not embed a JavaScript engine, bytecode VM, AST interpreter, source parser, `eval` interpreter, or general-purpose JS runtime that executes JS semantics dynamically at runtime. JavaScript constructs must be lowered at compile time into Wasm control flow, data operations, and calls.

For example, here's what the CLI API might look like:

```bash
ayeyaiyai test.js -o test.wasm
wasmtime test.wasm
```

You will know that you have successfully finished creating the compiler when it passes 100% of the [test262](https://github.com/tc39/test262) test suite for the `language` category using `wasmtime`.

## Immutable Rules

1. Do not stop to ask me questions
2. Do not stop to update me on your progress
3. Always keep going in pursuit of the [Goal](#goal) - If you feel like stopping, instead consider three more unexplored avenues that you can go down in pursuit of the [Goal](#goal), and go down them
4. Do not stop because you reached a milestone, produced a working baseline, got a green test run, or completed a reasonable checkpoint; those are signals to continue immediately with the next concrete step toward the [Goal](#goal)
5. Only stop if the [Goal](#goal) is fully achieved

## test262 Language Tracking

When a `test/language/...` test is fixed, rerun it and require it to pass through the direct JS->Wasm backend before marking it complete.
Then immediately in `test262-language-progress.md`:

- mark that exact test line with `[x]`
- immediately after `[x]`, insert an ISO 8601 timestamp with date and time down to the second in parentheses, before the test path text, for example: `[x] (2026-03-19T22:24:05-06:00) test/language/...`
- work on any individual checkbox for at most 1 hour; if it is still not fixed and passing through the direct JS->Wasm backend at that point, skip it and move on
- mark a skipped checkbox by leaving it unchecked and inserting a skip marker in the parentheses immediately after `[ ]` using an ISO 8601 timestamp, for example: `[ ] (SKIPPED 2026-03-19T23:24:05-06:00 after 1h) test/language/...`
- when choosing the next test to work on, ignore unchecked entries that already carry a `SKIPPED` marker and proceed to the next unchecked unskipped test
- skipped entries do not count as completed, do not change the completed/total counts, and do not participate in the rolling average calculation
- update the top progress line so it still contains `x/y (z%)` and then appends the rolling average time per completed checkbox over the last 10 completed checkboxes, expressed in minutes; for example: `45/23637 (0.19%) — avg(last 10): 3.42 min/check`
- immediately below that top progress line, maintain a `Sub-category progress` block at the top of `test262-language-progress.md` that lists every top-level `test/language` sub-category in tracker order, each showing `completed/total (percent%)`
- refresh the overall top progress line and every sub-category progress line every time any checkbox changes, so overall language completion and each sub-category completion are visible at a glance without scrolling
- compute that rolling average from the timestamps on the 10 most recently completed checked entries in `test262-language-progress.md`; if fewer than 10 timestamped checked entries exist, use all currently timestamped checked entries after the first timestamp and compute the average elapsed minutes between consecutive completions
- every time you add a new completion timestamp, recompute and refresh that rolling average on the top line immediately
- proceed to the next unchecked test

Do not mark tests as complete for any pass that used an intermediate source-generation compiler stage.
