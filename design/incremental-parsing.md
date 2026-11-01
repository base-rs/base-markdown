# Incremental Parsing

## Requirement

`IncrementalParser` accepts Markdown in arbitrary UTF-8 pieces and emits the semantic events of each document-level block once appending text can no longer change them.

- **Equivalence:** for any division of a source into pieces, the events emitted by all `push` calls followed by `finish` equal the events of `EventParser` for the whole source, excluding the document start and end events, provided that no reference or footnote call resolves to a definition in a later document-level block.
- **Independence of division:** the concatenated output is identical for every division of the same source, including sources with forward references.
- **No retraction:** each event is emitted exactly once, and only when no appended text can change it or any event before it. Every event emitted by `push` lies before `stable_offset`, which never decreases.
- **Reference scope:** inline content in a document-level block resolves references and footnote calls against the definitions that begin before the end of that block. A reference to a definition in a later document-level block stays literal text.
- **Block-level granularity:** stability is decided per document-level block. A block is emitted when a later document-level block begins on a line that no earlier state or lookahead can reinterpret, or when the complete lines end in a document-level blank line, or a clean ATX heading, thematic break or closed fence that closes every container. Lists, block quotes and other containers are emitted as a whole. When frontmatter is enabled and the first line opens frontmatter, all output is held back until the frontmatter closes; a first line that cannot open frontmatter, such as a code fence without an information string, holds nothing back.
- **Cost:** text without a new complete line performs no parsing. Each preparation restarts at an emitted document-level block whose line can be prepared without earlier state, so the work per completed line is proportional to the open tail of the document rather than its full length. Definitions before the restart point are borrowed from an incrementally indexed seed rather than copied or re-sorted, and finding complete lines scans only newly appended text. Work remains proportional to the length of a block that stays open, such as a long list, and of a run of blocks that depend on their predecessors, such as consecutive definitions.

## Rationale

Speech synthesis and other streaming consumers receive agent replies as text deltas and need to act on completed blocks before the reply ends, without reparsing the whole reply on every delta or retracting output they have already acted on. Equivalence with `EventParser` keeps one Markdown semantics across complete and streamed input. Forward references cannot be resolved before their definitions arrive; scoping references to earlier definitions keeps the output deterministic instead of depending on where the deltas happened to split.

Progressive events, emitted at character granularity inside open blocks with later resolution events for properties such as a paragraph's kind or a list's tightness, were specified in November 2026 for Tomori's speech synthesis (pull request #91) and are parked. Tomori consumes whole document-level blocks instead: a table, code block or list may be visualized as well as spoken, which needs its complete structure, and making character-level output reliable under GFM tables needed far more held constructs and resolution rules than the earlier speech latency justified (pull request #93). The cost is that speech waits for the first block to close.

## Scope

`IncrementalParser`, its interaction with block preparation (`Blocks::parse_from` and the recorded outline of document-level blocks), and definition lookup scoping. `EventParser`, `Document` and the HTML renderer are unchanged and continue to resolve forward references across the whole document.

## Validation

Run `tests/incremental_parser.rs`, which compares every two-piece division, character-by-character pushes and random multi-piece divisions with a single push and with `EventParser` for a corpus covering lists, nested lists, block quotes with lazy continuation, fenced and indented code, HTML blocks, tables after paragraphs, setext headings, link reference definitions, footnotes, frontmatter and CR, LF and CRLF line endings; for every `CommonMark` specification example; and for random documents built from block-opening, continuation and reinterpreting lines. Comparison with `EventParser` is limited to sources without definitions, where reference scope cannot differ; division independence is checked for all of them. The tests also check that emitted events lie before `stable_offset`, that forward references stay literal for every division, that most corpus events are emitted before `finish`, and that a first line which cannot open frontmatter holds nothing back. The unit tests in `src/incremental_parser.rs` compare random definition-heavy documents, including definitions in containers, multi-line titles, footnotes and frontmatter, with a one-shot parse whose reference scope is set per document-level block without anchors or seeds, and check that the restart point stays near the end of a long document of closed blocks.

When changing block preparation, review new lookahead across lines: a line whose classification depends on a later line must not be recorded as a clean document-level boundary, and a block that can be reinterpreted by a later line must not settle the outline.
