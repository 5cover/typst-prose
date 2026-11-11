# Roadmap

## Purpose

`typst-prose` extracts prose from Typst 0.15.1 documents, in two strategies that answer different questions:

- `source` answers "what did the author write?" It walks the syntax tree and never evaluates the document. Its defining invariant is that it does not run the code.
- `compiled` answers "what does the reader see?" It reads the rendered `PagedDocument`, so it gets values, generated labels, navigation and bibliography text that source cannot see.

Both strategies are measured against a hand-written corpus with explicit expectations (`fixtures/corpus/manifest.toml`). Expectations are the intended output. Where an extractor differs, the entry is recorded as a limitation with the observed output. The gate fails on regressions and on unexpected passes. The corpus is not a coverage claim for arbitrary documents.

Three kinds of entry are tracked separately:

- **Known limitations** are costs that the design accepts, or decisions that are recorded but not yet settled. Each has a reason, and changing it changes a design decision, not a bug.
- **Outstanding limitations** are defects with a planned fix, and none remain open except the one listed as infeasible below.
- **Infeasible** items have an attempt and an analysis recorded, so the next person can start from there.

## Current state

- Corpus gate: 62 fixtures, 71 exact-text checks, 194 sentinel checks, 15 `known` entries (source 11, compiled 4), 0 failures (`scripts/coverage.sh`).
- Fixture scripts: `run-fixtures.sh` (10 passed, 4 failing on purpose), `invariants.sh` (6 passed).
- Implemented: root resolution, installed packages (never downloaded), `datetime.today()` (fixed with `--date`, or the system clock), system fonts with `--font-path` and `--no-system-fonts`, the source content-value rule, and compiled word boundaries from the element structure.
- Smoke test: enssat-E1 (outside this repository) runs in both modes. Its outputs are kept under `target/out/` and are not committed.

## Done in the last round

Each item is a commit, with its corpus entry updated in the same commit.

- Source: a content block gets a space only where a letter or digit meets it, so `#lower[WORDS].` stays `WORDS.`.
- Compiled, clipping: a text run whose start lies outside a clip box is dropped.
- Compiled, repeated table cells: a cell that repeats a cell of the same table at the same grid position is dropped.
- Compiled, math: rendered math variables in U+1D400 to U+1D7FF are written as plain letters (NFKC, that range only).
- Compiled, quotes: a block quote is a block, and its attribution starts a unit of its own. Adjacent quotes stay apart.
- Compiled, lists: each item's marker starts a unit, and a marker is set apart from its text. Bullet glyphs are not written.
- Compiled, footnote numbers, outline numbers and bibliography labels: each is set apart from the text it introduces. A bibliography label and its entry form one unit.
- Compiled, fonts: the default lays out with the system fonts, as the Typst CLI does. `--no-system-fonts` gives output that does not depend on the machine.
- Compiled, explicit line breaks: a newline in both modes, as the source help states.

## Known limitations

These are accepted or decided. The corpus records each one.

Source mode:

- Tables, figures and block quotes written in a sentence join the sentence. Source cannot tell a block call from an inline call. Accepted.
- A footnote body joins its sentence. The walker does not match callee names, because a user definition can shadow `footnote`. Accepted.
- Two content blocks that touch are split, so the rendered `ab` becomes `a b`. A split word is preferred to a glued one. Accepted.
- The template text of an unused `#let` definition is reported as its own unit. Source reports markup wherever it is written. Accepted.
- Text computed from strings is invisible: numbering, supplements and displayed strings need evaluation. Part of the contract.
- Data files (`json`, `read`, `.yml`, `.bib`) are not read. Values from them stay placeholders. Decided: source must not evaluate or parse data.
- Quote attributions come before quote bodies, because source follows the argument list. This is the rule the walker implements.
- Trailing content of a call is inline, so a nested block joins the outer block's text. Same cause as the table case.

Compiled mode:

- Rendered math text is extracted as prose, so the math string appears in the output. Policy question, below.
- Rendered raw text is extracted as prose. Policy question, below.
- The reference number of a footnote is extracted before its note body, as a generated number. Policy question, below.
- Generated text that the document includes is extracted as rendered: filler from `lorem()`, the contents and figure list, and bibliography entries. Decided: compiled mode includes what the reader sees.
- Only installed packages are read. Packages are never downloaded, so the tool stays offline. Decided.

## Infeasible

### Source: `#linebreak()` between a label and its value

Symptom: in source mode, `#linebreak()` is a value expression, so the placeholder `[...]` is written between a label and its value. The boundary is a word boundary (or a newline), so the placeholder is wrong. Test: `compiled-gaps/grid-labels`, source exact check, which is a known limitation.

Attempts and why they do not work:

1. Match the callee name `linebreak`. This works for the common case, but a user definition can shadow the name, and the walker cannot resolve which binding a name refers to without evaluating the document. The footnote case was dropped for exactly this reason, so the same rule applies here.
2. Treat any value expression with no content and no arguments as a boundary. Wrong for `#term("x")`, `#h(1em)` and `#sym.nbsp`, which are not line breaks.
3. Treat the markup form `\` as the break. This is already done: the markup linebreak is a newline. It covers the markup form only, not the call form.

Next step, if the call form matters: the question is a policy for shadowable built-ins, not a code change. The options are to accept the placeholder, or to match the name and document the shadowing case as a known cost, as for the footnote.

## Outstanding

None that can be fixed within the current design. Items in the open decisions below need an answer before work on them can start.

## Open decisions

These do not change the code until decided:

- Math and raw text: keep them as prose, as now, or exclude them as source does. Excluding them is the source behaviour, and it would remove `MathQuoted`, `RawInlineWord` and `RawBlockWord` from the output.
- Footnote reference numbers: keep the generated number before the note body, or drop it, as the expectation for `boundaries/footnote` says.
- Font correction. The font spike is done, and it corrects two claims made in commit `51a2b36`. First, `Encode Sans` is installed on this machine: `typst fonts` lists it, and a width measurement with the default options gives 113.21 pt for a sample, against 104.76 pt for the embedded fallback. So the body text of the report is not a fallback here. Second, the template's `ENGIE` and `Red Hat Mono` are found through `--font-path`, and `ENGIE` also resolves with the default options. The template never requests `Burlingame` (it names `BurlingameW04` only in its file), so that family is not a gap. The open question is therefore whether the author's machine has `Encode Sans`, which decides whether the report's pagination can be reproduced here. `--no-system-fonts` remains the way to get output that does not depend on the machine.

## Next steps

Ordered by how much each one tells us about the real report.

1. Settle the open decisions above, then change the corresponding expectations in the same commit.
2. Run the font spike (family names in the template's font files).
3. Smoke-test enssat-E1 in both modes after each change, and compare with the previous output, to confirm that a change moves only the lines it should.

## Non-goals

- Source mode does not evaluate the document, and it does not read data files.
- No coverage claim for arbitrary documents. The corpus measures specific constructs in Typst 0.15.1.
- Packages are never downloaded.
- No attempt to reproduce Typst's layout, or to reconstruct math source.

## Maintenance

- Typst is pinned to 0.15.1. A version change needs a corpus re-run, and a review of every known entry, because layout changes can fix or break them.
- Expectations are written as intended semantics before the output is checked. A mismatch becomes a known entry with the observed output. Expectations are not changed to match output.
- When a limitation is fixed, its `known` entry is removed in the same commit, because the gate fails on unexpected passes.
- Every change runs `CARGO="cargo +stable-x86_64-pc-windows-gnu" bash scripts/coverage.sh`, plus `cargo test`, `run-fixtures.sh` and `invariants.sh`.
