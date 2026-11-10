#!/usr/bin/env bash
# Derives variants from fixtures by semantics-preserving edits and checks that
# the extracted prose is byte-identical to the unmodified fixture's output.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/debug/typst-prose"
work="$root/target/invariants"
mkdir -p "$work"
pass=0
fail=0

check() {
  local label="$1" base="$2" sed_expr="$3"
  local variant="$work/$(basename "$label" .typ).typ"
  sed "$sed_expr" "$base" > "$variant"
  if cmp -s "$variant" "$base"; then
    echo "ERROR $label (transform changed nothing)"
    fail=$((fail + 1))
    return
  fi
  local a b
  a="$("$bin" "$base" 2>/dev/null)" || { echo "ERROR $label (base failed)"; fail=$((fail + 1)); return; }
  b="$("$bin" "$variant" 2>/dev/null)" || { echo "ERROR $label (variant failed to compile)"; fail=$((fail + 1)); return; }
  if [ -n "$a" ] && [ "$a" == "$b" ]; then
    echo "PASS $label"
    pass=$((pass + 1))
  else
    echo "FAIL $label"
    diff <(echo "$a") <(echo "$b") | sed 's/^/    /'
    fail=$((fail + 1))
  fi
}

split="$root/fixtures/basic/page-split-paragraph.typ"
check "page-height-160pt" "$split" 's/height: 80pt/height: 160pt/'
check "page-height-50pt" "$split" 's/height: 80pt/height: 50pt/'
check "inline-emph-inside-word" "$split" 's/several/sev#emph[eral]/'
check "styling-boundary-mid-sentence" "$split" 's/The third sentence/The third #text(fill: red)[sentence]/'

heads="$root/fixtures/basic/headings-and-inline.typ"
check "styling-boundary-heading-inline" "$heads" 's/the first paragraph/the #text(fill: red)[first] paragraph/'
check "inline-boundary-in-sentence" "$heads" 's/The second paragraph/The sec#emph[ond] paragraph/'

echo "passed=$pass failed=$fail"
