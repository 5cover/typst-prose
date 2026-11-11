# Roadmap

## Purpose

`typst-prose` extracts prose from Typst 0.15.1 documents, in two strategies that answer different questions:

- `source` answers "what did the author write?" It walks the syntax tree and never evaluates the document. Its defining invariant is that it does not run the code.
- `compiled` answers "what does the reader see?" It reads the rendered `PagedDocument`, so it gets values, generated labels, navigation and bibliography text that source cannot see.

Both strategies are measured against a hand-written corpus with explicit expectations (`fixtures/corpus/manifest.toml`). Expectations are the intended output. Where an extractor differs, the entry is recorded as a limitation with the observed output, and the gate fails on regressions and on unexpected passes. The corpus is not a coverage claim for arbitrary documents.

Two kinds of limitation are tracked separately:

- **Known limitations** are costs that the design accepts, or decisions that are recorded but not reversed. They stay documented. Each has a reason, and changing it would change a design decision, not fix a bug.
- **Outstanding limitations** are defects. Each has a planned fix, a test in the corpus, and a note on what the fix needs first.

## Current state

- Corpus gate: 61 fixtures, 70 exact-text checks, 194 sentinel checks, 27 `known` entries, 0 failures (`scripts/coverage.sh`).
- Fixture scripts: `run-fixtures.sh` (10 passed, 4 failing on purpose), `invariants.sh` (6 passed).
- Implemented: root resolution, installed packages (never downloaded), `datetime.today()` (fixed with `--date`, or the system clock), compiled word boundaries from the element structure (explicit line breaks, term items, numbered headings), and the source content-value rule.

## Known limitations

These are accepted or decided. The corpus records each one.

Source mode:

- Tables, figures and block quotes written in a sentence join the sentence. Source cannot tell a block call from an inline call. Accepted.
- A footnote body joins its sentence. The walker does not match callee names, because a user definition can shadow `footnote`. Accepted.
- Two content blocks with no space between them are split, so the rendered `ab` becomes `a b`. A split word is preferred to a glued one. Accepted.
- The template text of an unused `#let` definition is reported as its own unit. Source reports markup wherever it is written, by design. Accepted.
- Text computed from strings is invisible: numbering, supplements and displayed strings need evaluation. Part of the contract.
- Data files (`json`, `read`, `.yml`, `.bib`) are not read, so values from them stay placeholders. Decided: source must not evaluate or parse data.
- Quote attributions come before quote bodies, because source follows the argument list. This is the rule the walker implements.

Compiled mode:

- Generated filler (`lorem`) is extracted, because compiled mode includes what the reader sees. Decided.
- The contents and figure list are extracted as rendered. Decided.
- Bibliography entries are extracted as rendered. Decided.
- Rendered raw text and math glyphs are extracted as prose. The policy question is open: exclude them as source does, or keep them and mark them.
- Only installed packages are read. Packages are never downloaded, so the tool stays offline. Decided.

## Outstanding limitations

Source mode:

- **Punctuation after a content block.** A space is written after every content block, so `#lower[LOWER WORDS].` gives `LOWER WORDS .`. The fix adds the space only between letters or digits. About 100 call-and-punctuation sites exist in the enssat-E1 report, so this is the most visible defect. Test: `compiled-gaps/case-transform`, which fails in source mode today.
- **`#linebreak()` in source mode.** It is a value expression, so it becomes the placeholder between a label and its value. A line break should be a word boundary. Test: `compiled-gaps/grid-labels`, source exact check.

Compiled mode:

- **Clipped text.** The clip rectangle is not applied, so text outside a clipped box is extracted. Test: `compiled-gaps/clip-hide`.
- **Repeated table headers.** A header that the layout repeats on a new page is extracted twice. Test: `compiled-gaps/table-repeat-header`.
- **Math italic letters.** Glyphs in U+1D400 to U+1D7FF come out as Unicode math letters rather than plain letters. Normalize that range only, because whole-string NFKC would also change U+00A0. Test: `boundaries/math`.
- **Quote attributions.** The attribution run is detached, so it has no span to match, and it runs into its body. Find a typed rule: the attribution is a field of the quote element.
- **Bullet and number markers.** They glue to their items. Probe whether the marker is a separate element or run.
- **Contents entries.** Their numbers glue to their titles. The entries are outline entries, not heading elements, so the boundary rule does not reach them. Probe whether an outline entry links to its heading.
- **Footnote reference numbers.** They glue to the note body. Probe the `entry` content for a number field.
- **Bibliography reference labels.** The label `[1]` is its own unit, apart from its entry. Test: `compiled-gaps/bibliography`.
- **Adjacent block quotes.** They run together. Test: `boundaries/quote`.
- **Font set.** Compiled mode embeds only the `typst-assets` fonts. The fluidssat template asks for Encode Sans, Arial, ENGIE, Red Hat Mono and Consolas. Unmeasured: whether line breaks or hyphenation change. Measure by compiling enssat-E1 with and without system fonts, then decide whether to load system fonts.

## Next steps

Ordered by how small and verifiable each fix is. Each step ends with the gate, `cargo test`, `run-fixtures.sh` and `invariants.sh`.

1. Fix source punctuation after content blocks. The gate should show `compiled-gaps/case-transform` passing in source mode, and the enssat-E1 output should lose its artificial spaces.
2. Apply the clip rectangle to text runs (outstanding: clipped text).
3. Deduplicate repeated table cells by location, after probing that repeated header cells share a `Location`. The generated-cells check must still pass.
4. Normalize math italic letters, for that Unicode range only.
5. Treat `#linebreak()` as a word boundary in source mode.
6. Probe and fix the remaining boundary cases: quote attributions, bullet and number markers, contents entries, footnote numbers, and bibliography labels.
7. Measure the font effect, then decide on system font loading.

Open decisions, to settle before the work they affect:

- Whether compiled mode excludes raw text and math glyphs, as source does, or keeps them and marks them. This decides step 4 and the math entry.
- Whether the footnote callee-name rule should come back for documents that never shadow the built-in. Today the cost is accepted for all documents.

## Non-goals

- Source mode does not evaluate the document, and it does not read data files.
- No coverage claim for arbitrary documents. The corpus measures specific constructs in Typst 0.15.1.
- Packages are never downloaded.
- No attempt to reproduce Typst's layout, or to reconstruct math source.

## Maintenance

- Typst is pinned to 0.15.1. A version change needs a corpus re-run, and a review of every known and outstanding entry, because layout changes can fix or break them.
- Expectations are written as intended semantics before the output is checked. A mismatch becomes a known or outstanding entry with the observed output. Expectations are not changed to match output.
- When an outstanding limitation is fixed, its `known` entry is removed in the same commit, because the gate fails on unexpected passes.
- The enssat-E1 report is an external smoke test, not part of the corpus. Its outputs are kept under `target/out/`, which is not committed.
