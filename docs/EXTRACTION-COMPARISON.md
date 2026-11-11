# Source syntax versus compiled extraction

Two strategies extract prose from a Typst 0.15.1 document:

- `typst-prose source` walks the syntax tree from `typst-syntax`. It does not evaluate anything. Markup is reported wherever it is written.
- `typst-prose compiled` reads the `PagedDocument` produced by compilation, as before.

Full contracts are in `typst-prose source --help` and `typst-prose compiled --help`. The numbers below come from `docs/COVERAGE.md`, which `scripts/coverage.sh` generates from the observed output of 50 fixtures (`fixtures/corpus`, expectations in `fixtures/corpus/manifest.toml`). Nothing here is an arbitrary-document claim.

## Results

| Measure | source | compiled |
| --- | ---: | ---: |
| Expected prose tokens recovered | 72 of 72 | 75 of 75 |
| Non-prose tokens included (the contract excludes them) | 0 | 0 |
| Exact-text checks passing (of 46 source and 36 compiled checks) | 35 | 36 |
| Documented known limitations (sentinel and exact-text) | 11 | 0 |
| Regressions or unexpected results | 0 | 0 |

Tokens recovered only by source (10): authored text that compiled mode does not render. These are unexecuted or hidden branches, unused functions and variables, an unused imported helper, a running header, the attribution of an inline quote, and text replaced by a show rule.

Tokens recovered only by compiled (13): string-derived or generated text, which source cannot see. These are displayed or argument strings, function results, a numbering pattern, reference supplements, a context string, and a string table cell.

Missed by both: none in the corpus. Source joins the footnote body to the sentence that contains the call, which is a known limitation. Compiled mode writes the footnote reference number before the body ("1 FootnoteBodyWord"), as a generated number that the reader sees.

## Which mode suits which task

Source mode gives the authored form, and it never evaluates the document. It recovers all 72 expected tokens. It includes text the reader never sees (unused definitions, template text, clipped and hidden text), and it does not emit strings, numbering, reference text, or raw and math content. For a checker that should review what the author wrote, it is the input, and it stays correct whatever the document computes.

Compiled mode gives what the reader sees, and for abstracted documents that is much more than the authored prose. Values computed from variables, data files and functions, generated labels and navigation, and bibliography entries exist only after compilation. Source mode sees a placeholder for each of them, and recovering the text would need evaluation, which source mode does not do. Compiled mode is the only route to that text, so it is the right input when the authored form hides the content.

Its limits are structural, and each one is a known, tested limitation:

- Source cannot tell a block call from an inline call. A trailing body such as `#block[...]` is joined to the surrounding text ("OuterBlock NestedBlock").
- Out-of-flow content is joined in place. Tables, figures, block quotes and footnote bodies are content blocks, so they join the sentence with a space on each side. The walker does not match callee names: a user definition can shadow a built-in such as `footnote`, and the walker cannot resolve which binding a name refers to without evaluating the document. The shadowing case is `content-values/footnote-shadowed`.
- Source order follows the argument list, so a block quote's attribution comes before its body.
- Text computed from strings is invisible. Numbering, supplements and displayed strings need evaluation.
- An element that renders no authored text (a value expression, a reference, math, raw text) is replaced by the placeholder `[...]` on the same line. The sentence stays one unit. With an empty placeholder the source spaces remain, and the sentence can read as ungrammatical.

Raw text and math are placeholders in compiled mode, as in source mode, because they are not erasable markup. Compiled mode separates the boundaries that the element structure marks: explicit line breaks, term items, numbered headings, list and enum markers, footnote and outline numbers, quote attributions, and bibliography labels. It also drops text outside a clip box and repeated table cells. It is weaker where the structure does not mark a boundary, and it reports rendered raw text as prose. Rendered math text is prose by decision. Its caveats are in `--help`, and the corpus records each one as a known limitation.

Compiled output also depends on the fonts of the machine that runs it, because line breaks and page numbers depend on them. `--no-system-fonts` gives output that is the same on every machine.

### Practical comparison on enssat-E1

The same report (`src/main.typ` of the enssat-E1 project, with `--root` at the package), run in both modes from the committed binary:

| Measure | source | compiled |
| --- | ---: | ---: |
| Units (blank-line blocks) | 200 | 384 |
| Distinct words | 1929 | 2228 |
| Words found only in this mode | 62 | 361 |
| Value placeholders `[...]` | 108 | 0 |
| Contents and figure-list lines (dot leaders) | 0 | 73 |
| URL lines (from the bibliography) | 0 | 22 |
| Heading numbers glued to their titles (contents entries only, after the boundary rule) | 0 | 4 |

The 62 words found only in source are template text that the report never renders: a placeholder `TBD`, the declaration form, and glossary definitions that source writes as one unit. The 361 words found only in compiled mode are the resolved values (where source shows `la [...] (direction`, compiled shows `la DSI (direction`), the contents and figure list, bibliography entries, and generated figure numbers. Both modes give 5 units that start in lowercase.

## What evaluation adds

When Typst evaluates the document, the additional prose is:

- Strings that are displayed, passed to functions, returned by functions, used as captions, supplements, table cells, show replacements, or produced by a context block.
- Generated numbers and prefixes, including heading numbering (`Qorvbu` appears in every numbered heading) and reference text ("See RefSupplementWord 1").
- Repeated output from a single authored definition. A closure that builds two cells gives two rendered copies, and the corpus checks the count (`GeneratedCell`: source 1, compiled 2).

Evaluation also removes text that source mode keeps: unexecuted branches, unused functions, and replaced show-rule text. It is the reason the two modes disagree on those tokens.

## Policy the source walker implements

These are the rules `typst-prose source --help` states. They are the basis of the boundary tests.

- A paragraph break, heading, list item, enum item or term item starts a new unit.
- A content block is an inline value wherever it is written: in markup, as a trailing argument, or as any other argument. Where a letter or digit meets the block on either side, one space is written, so words never join. Punctuation keeps the source spacing, so `#lower[WORDS].` stays `WORDS.`. The block adds no blank line, because the source does not show whether the call places it in a paragraph.
- No callee name is special. A footnote call, `#footnote[...]`, is an inline value like any other content block.
- Inline elements that render no authored text (value expressions, references, math, raw text, links) are replaced by the placeholder on the same line. Only statements that produce no text end the unit. Placeholders separated only by whitespace collapse to one. A paragraph made only of placeholders produces no unit.
- Escapes and shorthands are decoded. Only ASCII whitespace is collapsed, so non-breaking and thin spaces are kept. Soft hyphens are removed.
- Imports and includes with a plain string-literal path are followed by parsing, never by execution. Each imported module is walked once. Each include is walked at each occurrence. Variable paths, package paths and escaped paths are reported on stderr and not followed.

## Limits of this study

- 50 hand-written fixtures. They show the behaviour of these constructs in Typst 0.15.1. They do not measure coverage of arbitrary documents.
- Packages are not followed, and neither are dynamic import paths.
- The compiled extractor is unchanged. Its limitations are recorded, not fixed, as the task required. Each is a known limitation in the manifest with a reason.
- Expectations are the intended semantics written from Typst behaviour, then checked against real output. No expected value was changed to match output. Mismatches that reflect a real extractor defect are documented known limitations, with the observed output recorded. One fixture was changed: the quote attribution cases now use `block: true`, because Typst does not render the attribution of an inline quote, and the sentinel was meant to test a rendered attribution.

## Compiled extractor: frame facts, gaps and open work

These facts about the `PagedDocument` in Typst 0.15.1 explain the compiled extractor's behaviour and limits. They come from the feasibility study that preceded the source strategy. The evidence paths point to fixtures and probes that are still in the repository. The measured status of each gap is in `docs/COVERAGE.md`.

Frame facts:

- Tags present in the frame: `par`, `heading`, `figure`, `caption`, `table`, `cell`, `enum`, `item` (partial), `footnote` (reference), `entry` (footnote body), `ref`, `link-marker`, `equation`, `strong`, `emph`, `place`, `quote`, `artifact` and `pdf-marker-tag`. The bodies of `block` and `quote` are not wrapped in a `par`. Evidence: traces of all fixtures.
- A paragraph that crosses a page is one `par` start on the first page and one end on the second, with no page-separator item between them. Evidence: `fixtures/basic/page-split-paragraph.typ`.
- Runs keep their own leading and trailing spaces. Layout line breaks and explicit `\` produce no text, so the space between words can be lost. The frame has no line structure (a paragraph with an explicit break is one `par` with two text items), so the break is read from the element tree instead: the `linebreak` element among the `par` body's children. The extractor writes one space at each break, at each term and description boundary, and at the number of a numbered heading. Evidence: `probes/reorder-and-quote.typ`, `fixtures/corpus/tables/tables.typ`, `fixtures/corpus/boundaries/paragraphs.typ`, `fixtures/corpus/compiled-gaps/grid-labels.typ`.
- Automatic hyphenation inserts U+00AD at the break. Removing it rejoins the word. Only one fixture was tested. Evidence: `fixtures/basic/hyphenation.typ`.
- Inline math is emitted as separate Unicode math glyph runs with no spacing, so the source form is lost. Evidence: `fixtures/structure/math-inline.typ`.
- Bullets and numbers are text runs inside `pdf-marker-tag`. That tag also wraps tight list item bodies, so markers cannot be separated by tag alone. `detached` spans mark some generated runs (quote attribution) but not others: heading numbers have source spans, and a caption's authored words are detached too. No single signal is reliable. Evidence: `fixtures/structure/lists.typ`, `fixtures/structure/figure-caption.typ`.
- Header and footer text is inside `artifact` and is suppressed. Image alt text is reachable from `FrameItem::Image` through `Image::alt()`, but it is not body prose. Evidence: `fixtures/structure/page-header.typ`, `fixtures/structure/image-alt/figure.typ`.
- The footnote body is an `entry` unit, found through `GroupItem::parent`, which points to the footnote's location. The in-text marker sits in a `footnote` tag and is suppressed. The marker inside the body is not removed. Evidence: `fixtures/structure/footnote.typ`.
- A show rule that replaces text puts the replacement in the frame, with the rule's span. Text produced by a function call carries the span of the call, not the generated words. `src=detached` and `src=none` mean the provenance is unresolved. Evidence: `fixtures/contextual/show-rules.typ`, `probes/paragraph-across-pages.typ`.
- A caption is a `caption` tag that is independent of its figure's shape. Table cells come in source order. Two-column content came out in source order in the one case tested. A placed element at the bottom of a page was emitted first, which matched source order in the one probe. Evidence: `fixtures/structure/figure-caption.typ`, `fixtures/structure/table-cells.typ`, `fixtures/structure/columns.typ`, `fixtures/structure/placed-float.typ`.
- Heading numbers and reference text are generated. Their presence in the frame is correct, but the space between the number and the title is missing. Evidence: `fixtures/contextual/counters-and-refs.typ`.

Upstream work and open questions:

- The feasibility study recommended two small upstream changes that would close most of the compiled gaps: a paragraph-like tag around `block` and `quote` bodies, and an item-level tag for each list and enum item.
- Open: whether the element `Content` carried by `Tag::Start` has enough source structure to recover math source and generated markers. The compiled extractor does not use it. This needs a spike before choosing between an external extractor and a compiler integration.

Tested in `compiled-gaps/`, with the result recorded as known where the extractor differs:

- Clipped text: text that starts outside a clip box is dropped. A run that starts inside the box is kept whole. Hidden text with `hide()` is not extracted, as intended.
- Case transforms: `upper` and `lower` render the transformed case, and compiled mode extracts it. Source mode keeps the authored case.
- A table header that the layout repeats on a second page: the repeated cells are dropped, because they share a table and grid position with the cells they repeat.
- Bibliography from a YAML file: compiled mode extracts each entry with its label, as one unit.
- Reading order of two columns with unequal content: the order matches the source in the case tested.

Not yet tested:

- Smart quotes in a language that changes them (for example `set text(lang: "fr")`).
- Typst versions other than 0.15.1.

## Reproducing

```
CARGO="cargo +stable-x86_64-pc-windows-gnu" bash scripts/coverage.sh
```

On this machine the default toolchain cannot link. The GNU toolchain needs the `LIBRARY_PATH` workaround described in the README under "Building on Windows". Without the override, `cargo test --test corpus` runs the same harness.
