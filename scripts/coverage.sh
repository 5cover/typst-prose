#!/usr/bin/env bash
# Runs both extraction modes on every fixture in fixtures/corpus and checks the
# expectations in fixtures/corpus/manifest.toml (see tests/corpus.rs).
#
# Writes target/coverage/report.md and copies it to docs/COVERAGE.md.
# Exit status is nonzero for regressions, unexpected passes, fixture errors and
# manifest or corpus layout problems. Documented known limitations do not fail.
#
# Override the cargo command when the default toolchain cannot link, for example:
#   CARGO="cargo +stable-x86_64-pc-windows-gnu" bash scripts/coverage.sh
set -u
workers=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --workers)
      if [ "$#" -lt 2 ]; then
        echo "usage: $0 [--workers N]" >&2
        exit 2
      fi
      workers="$2"
      shift 2
      ;;
    --help|-h)
      echo "usage: $0 [--workers N]"
      echo "Run the full corpus using N parallel CLI workers (default: up to 4)."
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      echo "usage: $0 [--workers N]" >&2
      exit 2
      ;;
  esac
done

if [ -n "$workers" ]; then
  case "$workers" in
    *[!0-9]*|0*)
      echo "--workers must be a positive integer" >&2
      exit 2
      ;;
  esac
  export CORPUS_WORKERS="$workers"
fi

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root" || exit 1

# shellcheck disable=SC2086
${CARGO:-cargo} test --test corpus -- --nocapture
status=$?

if [ -f target/coverage/report.md ]; then
  cp target/coverage/report.md docs/COVERAGE.md
  echo "report: docs/COVERAGE.md"
fi
exit $status
