#!/usr/bin/env bash
# Compiles each fixture .typ, extracts prose (compiled mode), and diffs against the .expected.txt
# next to it. Prints PASS/FAIL per fixture and a summary. Set VERBOSE=1 to show diffs.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="$root/target/debug/typst-prose"
pass=0
fail=0
for typ in $(find "$root/fixtures" -path "$root/fixtures/corpus" -prune -o -name '*.typ' -print | sort); do
  expected="${typ%.typ}.expected.txt"
  name="${typ#$root/fixtures/}"
  if [ ! -f "$expected" ]; then
    echo "SKIP $name (no expected file)"
    continue
  fi
  actual="$("$bin" compiled "$typ" 2>"$root/target/fixture-stderr.txt")"
  if [ "$actual" == "$(cat "$expected")" ]; then
    echo "PASS $name"
    pass=$((pass + 1))
  else
    echo "FAIL $name"
    fail=$((fail + 1))
    if [ "${VERBOSE:-0}" == "1" ]; then
      diff <(echo "$actual") "$expected" | sed 's/^/    /'
    fi
  fi
  if [ -s "$root/target/fixture-stderr.txt" ]; then
    sed "s/^/    stderr: /" "$root/target/fixture-stderr.txt"
  fi
done
echo "passed=$pass failed=$fail"
