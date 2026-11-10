# Typst prose extraction: feasibility findings

Scope: prototype investigation of whether Typst's compiled `PagedDocument` (Typst 0.15.1)
contains enough semantic information to extract grammatical prose without geometric
reconstruction. Typst itself was not modified.

## Verdict

Outcome B from the plan, with two gaps that point toward a small integration.

- Paragraph grouping, logical order, paragraphs split across pages, inline style
  boundaries, captions, table cells, footnote bodies, and page headers and footers were
  all recovered from tags and text runs alone. No coordinates were read. Image alt text is
  reachable through the frame item but is not emitted, since it is not body prose.
- Gaps found, each reproduced by a fixture or probe:
  1. Text inside `block[...]` and `quote` bodies is not wrapped in a `par` tag, so it has
     no paragraph boundary.
  2. List and enumeration item boundaries are not individually tagged in the cases tested.
  3. Spaces that come from layout gaps are not in the text (heading numbers, quote
     attribution), so adjacent runs are glued together.
  4. Math renders as Unicode math glyphs with no spacing, so the source form is lost.
  5. Generated markers (bullets, numbers, footnote references) are not reliably separable
     from authored prose using tags alone.

Gaps 1, 2 and 3 look like targeted upstream fixes (tagging and separator markers). Gap 4
and the marker part of gap 5 may need access to the element `Content` carried by
`Tag::Start`, which this prototype does not use yet (see "Not tested").

## Build environment (reproducibility)

- Typst crates pinned exactly: `typst`, `typst-layout`, `typst-library`, `typst-syntax`,
  `typst-assets` at `=0.15.1` (see `Cargo.toml`). `Cargo.lock` is committed.
- The machine has no MSVC linker (no Visual Studio Build Tools). The build uses the
  `stable-x86_64-pc-windows-gnu` host toolchain (installed with rustup, user-level) and
  LLVM-MinGW from WinGet. Two environment-only workarounds were needed:
  - `LIBRARY_PATH` pointing to a directory with a stub `libgcc.a` and a copy of LLVM's
    `libunwind.a` renamed to `libgcc_eh.a`.
  - The LLVM-MinGW `bin` directory on `PATH`.
  These are not committed. Build command used:
  `LIBRARY_PATH=/tmp/mingw-stubs cargo +stable-x86_64-pc-windows-gnu build`
- The plan's `typst::compile::<PagedDocument>()` takes a `&dyn World` argument in 0.15.1,
  and `PagedDocument` lives in `typst-layout` (not re-exported by `typst`). Both are
  reflected in the code.

## Usage

```
typst-prose <input.typ>            # plain prose to stdout; warnings to stderr
typst-prose <input.typ> --trace    # frame traversal trace
bash scripts/run-fixtures.sh       # VERBOSE=1 for diffs
bash scripts/invariants.sh         # semantics-preserving variants must match
```

Code layout: `src/world.rs` (minimal `World`, bundled fonts, project-root file loading),
`src/trace.rs` (Phase A diagnostic trace), `src/extract.rs` (extractor).

## Extractor rules (as implemented)

- Units open at `par`, `heading`, `caption`, `item`, `cell`, `entry` (footnote body) and
  `place` tags, and collect every text run inside them, nested or not.
- Text outside any unit is emitted as its own unit in position order and reported on
  stderr as `untagged text`. It is not dropped and not merged across unit boundaries.
- Runs under `artifact` (page furniture) and `footnote` (the in-text reference marker) are
  suppressed and counted on stderr.
- U+00AD (soft hyphen, inserted at automatic line-break hyphenation) is removed.
- Whitespace inside a unit is normalized. Spaces at line ends are already part of the runs.
- Not implemented: separation of markers, use of `GroupItem::parent`, math reconstruction.

## Answers to the plan's questions

Evidence refers to fixtures in `fixtures/` and probes in `probes/`.

| Q | Answer | Evidence |
|---|---|---|
| Q1 | Yes in tested cases: every fixture's output equals its expected text, so no duplication or omission. | All fixtures, `scripts/run-fixtures.sh` |
| Q2 | In tested cases, frame order matched source order, including a `place(bottom)` element that is physically at the bottom of the page but emitted first. One probe only, so this is not proof of logical order in general. | `probes/reorder-and-quote.typ`, `fixtures/structure/placed-float.typ` |
| Q3 | Nested groups in sequence matched expected reading order in all fixtures except the enum case, where item boundaries are missing (Q22/Q29). | `lists.typ`, `columns.typ` |
| Q4 | `GroupItem::parent` is populated for footnote bodies: the body is physically at the page bottom and its parent is the footnote location. The extractor did not need it for the tested cases. | `fixtures/structure/footnote.typ` trace |
| Q5 | A paragraph crossing a page is one `par` Start on page 1 and one End on page 2, with no page-separator item in between. | `fixtures/basic/page-split-paragraph.typ` |
| Q6 | Tags were always paired and nested in traversal order across all 16 documents, including multi-page ones. | `# end` lines of `--trace` |
| Q7 | Tagged: `par`, `heading`, `figure`, `caption`, `table`, `cell`, `enum`, `item` (partial), `footnote` (reference), `entry` (footnote body), `ref`, `link-marker`, `equation`, `strong`, `emph`, `place`, `quote`, `artifact`, `pdf-marker-tag`. NOT tagged as paragraphs: `block` bodies and `quote` bodies. Quotation paragraphs were not tested beyond the single probe. | Traces of all fixtures |
| Q8 | Yes. A paragraph split over pages is one unit. | Q5 |
| Q9 | Yes. Runs keep their own trailing and leading spaces. | `page-split-paragraph.typ` runs |
| Q10 | Automatic hyphenation inserts U+00AD at the end of the first part of a broken word. Removing U+00AD rejoins the word correctly. Only one fixture was tested. | `fixtures/basic/hyphenation.typ` |
| Q11 | Partly. Neither kind of line break appears in the text, so layout line breaks and explicit `#linebreak()` are both invisible. Joining runs without a separator gave the correct word in the explicit case. Not tested: a line-break character in run text. | `probes/reorder-and-quote.typ` |
| Q12 | Yes. Strong, emphasis, and styling boundaries inside a paragraph do not split the unit, and the text is unchanged. | Invariant variants in `scripts/invariants.sh` |
| Q13 | No. Inline math renders as separate glyph runs with Unicode math italics and no spaces. The output is "E=mc2" style, not the source form. | `fixtures/structure/math-inline.typ` (known failure) |
| Q14 | Ordering: yes in tested cases, without coordinates. Inter-run spacing: no. Spaces that come only from layout gaps are absent, so some sentences are glued. | `counters-and-refs.typ`, `reorder-and-quote.typ` |
| Q15 | Yes for the tested contextual content: a reference, a show rule replacement, and heading numbers are present in the final frame text. | `counters-and-refs.typ`, `show-rules.typ` |
| Q16 | Yes in the tested cases: the reference reads "Section 1" consistent with the final introspection state. | `counters-and-refs.typ` |
| Q17 | Yes. References and heading numbering are included. The heading number and title are glued (known failure). | `counters-and-refs.typ` |
| Q18 | Partly. `pdf-marker-tag` wraps bullets and numbers, but also wraps tight list item bodies, so it cannot separate markers. `detached` spans mark some generated runs (quote attribution) but not others (heading numbers have source spans; a caption's authored words are detached too). `footnote` isolates the in-text marker. No single signal is reliable. | Traces of `lists.typ`, `figure-caption.typ`, `counters-and-refs.typ`, `reorder-and-quote.typ` |
| Q19 | Yes. Headers and footers are wrapped in `artifact`, and their text is suppressed. Authored text was not removed in the fixtures. | `fixtures/structure/page-header.typ` |
| Q20 | Not conclusively. Bullets and numbers are text runs inside `pdf-marker-tag`. Table rules and similar decoration are shapes. | `lists.typ` trace |
| Q21 | Yes for show rules that replace text: the replacement text appears in the final frame with the show rule's source span. No knowledge of the rule was needed. | `fixtures/contextual/show-rules.typ` |
| Q22 | Table cell order follows source order, and each cell is a tagged `cell`. Multi-line cells were not tested. | `fixtures/structure/table-cells.typ` |
| Q23 | Yes. The `caption` tag is a sibling of the shape and is independent of its contents. | `fixtures/structure/figure-caption.typ` |
| Q24 | Yes for one two-column case: column order matched source order. | `fixtures/structure/columns.typ` |
| Q25 | Custom containers using `block` produce untagged text (see Q7). They are reported, not lost. | `fixtures/contextual/show-rules.typ` |
| Q26 | Yes. The footnote body is an `entry` unit found through `GroupItem::parent`. The in-text marker is suppressed. The marker inside the body is not removed (known failure). | `fixtures/structure/footnote.typ` |
| Q27 | Not tested. `GroupItem::clip` and text under clipping were not examined. | None |
| Q28 | Yes. `FrameItem::Image` carries the image, and `Image::alt()` returns the alt text. The extractor does not emit it, which is correct since alt text is not body prose. | `fixtures/structure/image-alt/figure.typ` (trace shows the alt value) |
| Q29 | Classes that need more than tags and runs: block-level text without `par`; list and enum item boundaries; spacing from layout gaps; math source form; generated markers that share a tag with prose. | Gaps above |
| Q30 | Yes for authored text. Each glyph carries a span that resolves to a source range. | Trace `src=` fields |
| Q31 | Text from a function or show rule carries the span of the call or rule, not the generated words. Example: `lorem(120)` runs map to the call site; the show-rule replacement maps to the rule. | `probes/paragraph-across-pages.typ`, `show-rules.typ` |
| Q32 | Yes. One paragraph maps to several source ranges (a run for each styled part). | `show-rules.typ` trace |
| Q33 | Not conclusively tested. Show rules produce spans to the rule; the smart quote and case-transform cases were not fixtures. | `show-rules.typ` |
| Q34 | Yes. Unresolved provenance is explicit: `src=detached`, `src=none`. | Trace output |

## Coverage matrix

| Structure | Status | Notes |
|---|---|---|
| Paragraphs, incl. across pages | Reliable | Q5, Q8 |
| Headings (text) | Reliable | |
| Heading numbers | Gap (spacing) | "1.Methods" |
| Inline styling, links | Reliable | Invariant tests |
| Hyphenation | Reliable after U+00AD removal | Q10 |
| Explicit line breaks | Reliable (by joining) | Q11 |
| References and numbering | Reliable | "Section 1" |
| Show rules replacing text | Reliable | Q21 |
| Footnote body | Reliable | `entry` plus `GroupItem::parent` |
| Footnote reference marker | Suppressed | |
| Footnote body marker | Gap | Superscript marker inside `entry` remains |
| Figure caption | Reliable | |
| Figure contents, image alt | Excluded from prose | Alt reachable via `Image::alt()` |
| Table cells | Reliable in tested case | |
| Lists (nested, tight) | Gap | Markers emitted; item boundaries absent in enum |
| Multicolumn | Reliable in tested case | |
| Placed element | Reliable in tested case | |
| Inline math | Gap | Unspaced glyphs, Unicode math italics |
| `block` body text | Gap | Untagged; emitted as its own unit |
| Block quotation | Gap | Untagged body; attribution has no space before it |
| Page header and footer | Suppressed | `artifact` |

## Confirmed limitations (with examples)

1. Block text has no paragraph tag. `fixtures/contextual/show-rules.typ`: `block[Note: ...]`
   yields an untagged run. Output is complete, but the boundary is the extractor's guess.
2. Enum items share one container. `fixtures/structure/lists.typ`: "1.Numbered one2.Numbered two"
   is one unit because the enumeration has no per-item boundary.
3. Markers stay in prose. Bullets appear as "••Second item". Pure markers cannot be removed
   by tags without guessing, because tight list bodies share the `pdf-marker-tag` wrapper.
4. Layout spacing is lost. "1.Methods" (heading number), "prose.- Author" (quote attribution).
5. Math source is lost. `fixtures/structure/math-inline.typ` gives Unicode math glyphs with no spacing.
6. Footnote body marker remains: "1Source text inside the footnote."

## Recommendation

Continue with the external extractor on `PagedDocument` for the current architecture, since
none of the gaps required coordinates. Before extending it, pursue two upstream-facing
changes that would close most gaps with small compiler changes: a paragraph-like tag around
`block` and `quote` bodies, and an item-level tag for every list and enum item. The
remaining question is whether the element `Content` in `Tag::Start` gives enough source
structure for math and for markers; that needs a follow-up spike before deciding between
an external extractor and a compiler integration.

## Not tested

- Quotations beyond one probe, nested blocks and boxes, multi-line table cells, tables
  spanning pages, explicit page breaks, clipped or overlaid text (Q27), smart quotes and
  case transforms (Q33), and repeated headers over many pages.
- Source-form recovery from `Tag::Start` content (not used by the prototype).
- Typst versions other than 0.15.1.
- Non-ASCII text beyond the Latin fixtures.
- Duplicate-free output under layout iteration beyond the fixtures compared.

## Deviations from the plan

- Phases C to F: fixtures for contextual and structural cases were written from author
  intent. The expected files were not adjusted to match output, so failing fixtures stay
  as documented gaps. One fixture (footnote) was written with the footnote body as its own
  unit, which is the design chosen for this prototype rather than the inline form first
  written. Two fixtures were corrected for my own source errors (a doubled "Section" word
  and a placed-note order), and the expected files were updated to match the corrected source.
- The untagged-text policy (emit and report, rather than drop or silently merge) is a
  judgment call made for this prototype.
