#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARTIFACT_DIR="$ROOT_DIR/.artifacts/test262"
DEFAULT_CACHE_DIR="$ROOT_DIR/.cache/test262"

print_usage() {
  cat <<'EOF'
Usage:
  ./test262.sh [--category <name>]... [--test <path>]... [runner options...]

Categories:
  language
  built-ins
  builtins
  intl402
  annexB
  annex-b
  staging
  implementation-contributed
  all

Examples:
  ./test262.sh
  ./test262.sh --category language
  ./test262.sh --test test/language/expressions/yield/rhs-iter.js
  ./test262.sh --test language/expressions/yield/rhs-iter.js
  ./test262.sh --category built-ins --limit 50
  ./test262.sh --category language --category built-ins
  ./test262.sh --contains 'test/language/statements/'
EOF
}

append_category_filters() {
  local category="${1,,}"

  case "$category" in
    all)
      ;;
    language)
      CATEGORY_FILTERS+=("--contains" "test/language/")
      ;;
    built-ins|builtins)
      CATEGORY_FILTERS+=("--contains" "test/built-ins/")
      ;;
    intl402)
      CATEGORY_FILTERS+=("--contains" "test/intl402/")
      ;;
    annexb|annex-b)
      CATEGORY_FILTERS+=("--contains" "test/annexB/")
      ;;
    staging)
      CATEGORY_FILTERS+=("--contains" "test/staging/")
      ;;
    implementation-contributed)
      CATEGORY_FILTERS+=("--contains" "test/implementation-contributed/")
      ;;
    *)
      printf 'unknown category: %s\n\n' "$1" >&2
      print_usage >&2
      exit 2
      ;;
  esac
}

resolve_test262_dir() {
  if [[ -n "${TEST262_DIR:-}" ]]; then
    printf '%s\n' "$TEST262_DIR"
    return
  fi

  if [[ -d "$ROOT_DIR/test262/test" ]]; then
    printf '%s\n' "$ROOT_DIR/test262"
    return
  fi

  if [[ -d "/tmp/test262/test" ]]; then
    printf '%s\n' "/tmp/test262"
    return
  fi

  printf '%s\n' "$DEFAULT_CACHE_DIR"
}

TEST262_DIR_RESOLVED="$(resolve_test262_dir)"

if [[ ! -d "$TEST262_DIR_RESOLVED/test" ]]; then
  mkdir -p "$(dirname "$TEST262_DIR_RESOLVED")"
  git clone --depth 1 https://github.com/tc39/test262.git "$TEST262_DIR_RESOLVED"
fi

mkdir -p "$ARTIFACT_DIR"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
LOG_PATH="$ARTIFACT_DIR/run-$TIMESTAMP.log"
LATEST_LOG_PATH="$ARTIFACT_DIR/latest.log"

CATEGORY_FILTERS=()
EXACT_TESTS=()
PASSTHROUGH_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --category)
      shift
      if [[ $# -eq 0 ]]; then
        printf '--category requires a value\n\n' >&2
        print_usage >&2
        exit 2
      fi
      append_category_filters "$1"
      ;;
    --category=*)
      append_category_filters "${1#*=}"
      ;;
    --test)
      shift
      if [[ $# -eq 0 ]]; then
        printf '%s\n\n' '--test requires a value' >&2
        print_usage >&2
        exit 2
      fi
      EXACT_TESTS+=("--test" "$1")
      ;;
    --test=*)
      EXACT_TESTS+=("--test" "${1#*=}")
      ;;
    --list-categories)
      print_usage
      exit 0
      ;;
    --help|-h)
      print_usage
      exit 0
      ;;
    *)
      PASSTHROUGH_ARGS+=("$1")
      ;;
  esac
  shift
done

RUNNER_CMD=(
  cargo run --release --bin test262 -- --test262-dir "$TEST262_DIR_RESOLVED"
  --timeout-seconds "${TEST262_TIMEOUT_SECONDS:-5}"
)

if [[ ${#CATEGORY_FILTERS[@]} -gt 0 ]]; then
  RUNNER_CMD+=("${CATEGORY_FILTERS[@]}")
fi

if [[ ${#EXACT_TESTS[@]} -gt 0 ]]; then
  RUNNER_CMD+=("${EXACT_TESTS[@]}")
fi

if [[ ${#PASSTHROUGH_ARGS[@]} -gt 0 ]]; then
  RUNNER_CMD+=("${PASSTHROUGH_ARGS[@]}")
fi

printf 'test262 directory: %s\n' "$TEST262_DIR_RESOLVED"
printf 'log file: %s\n\n' "$LOG_PATH"

set +e
"${RUNNER_CMD[@]}" | tee "$LOG_PATH"
RUN_STATUS=${PIPESTATUS[0]}
set -e

cp "$LOG_PATH" "$LATEST_LOG_PATH"

SUMMARY_LINE="$(grep '^SUMMARY ' "$LOG_PATH" | tail -n 1 || true)"
if [[ -n "$SUMMARY_LINE" ]]; then
  DISCOVERED="$(sed -nE 's/.* discovered=([0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  ATTEMPTED="$(sed -nE 's/.* attempted=([0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  PASSED="$(sed -nE 's/.* passed=([0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  COMPILE_FAILED="$(sed -nE 's/.* compile_failed=([0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  RUNTIME_FAILED="$(sed -nE 's/.* runtime_failed=([0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  SKIPPED_METADATA="$(sed -nE 's/.* skipped_metadata=([0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  SKIPPED_CONTENT="$(sed -nE 's/.* skipped_content=([0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  ATTEMPT_RATE="$(sed -nE 's/.* attempt_rate_percent=([0-9]+\.[0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"
  COMPLIANCE="$(sed -nE 's/.* compliance_percent=([0-9]+\.[0-9]+).*/\1/p' <<<"$SUMMARY_LINE")"

  printf '\n'
  printf 'Compliance: %s%% (%s / %s discovered tests passed)\n' "$COMPLIANCE" "$PASSED" "$DISCOVERED"
  printf 'Attempt rate: %s%% (%s / %s discovered tests executed)\n' "$ATTEMPT_RATE" "$ATTEMPTED" "$DISCOVERED"
  printf 'Failures: compile=%s runtime=%s\n' "$COMPILE_FAILED" "$RUNTIME_FAILED"
  printf 'Skipped: metadata=%s content=%s\n' "$SKIPPED_METADATA" "$SKIPPED_CONTENT"
  printf 'Latest log: %s\n' "$LATEST_LOG_PATH"
fi

exit "$RUN_STATUS"
