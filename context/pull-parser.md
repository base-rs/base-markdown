# Pull Parser Architecture

The parser and document traversal produce the same pull-based semantic event stream. HTML rendering and AST construction consume that stream without first collecting it. This document describes the implementation. `Document` is the sole AST; [Document AST and migration](document.md) describes editing and the removed mdast APIs.

```text
source + parser workspace → semantic events → HTML writer → output
                                           → AST builder → document
document traversal        → semantic events → the same HTML writer
```

## Contract

An event describes resolved Markdown meaning: a container opens or closes, text occurs, or a leaf such as a break occurs. It never exposes speculative delimiters, tokenizer states, or event-buffer edits. A producer implements `Iterator<Item = Result<Event, Error>>`; consumers pull only the events they need. Start events contain the metadata required to render their opening tags, including resolved destinations and list tightness. End events identify the container that closes. Events carry byte ranges for provenance independently of their semantic values. The root is a single document; container ends match their starts and cannot precede the opening source extent. Heading and footnote-definition capture cannot be nested within a heading or image alternative text. Consumers and checked AST edits enforce these constraints.

HTML rendering must satisfy this invariant with identical source and rendering options:

```text
render(parser.events()) == render(build_ast(parser.events()).events())
```

No component between producer and consumer collects the complete semantic stream. The AST builder accepts a mutable iterator and an event budget, preserving its open-node stack between calls. Exhausting a budget is different from reaching the end of the document. The HTML writer writes directly to caller-owned output storage. Stopping consumption is supported; producer preparation and already-completed work are not undone.

## Source storage and editing

A document owns a caller-supplied UTF-8 source buffer. A source-backed text value contains byte offsets, not pointers into the document's allocation. Moving or reallocating an otherwise unchanged source does not invalidate offsets. Source ownership is independent of the parser's lifetime; events themselves do not borrow parser scratch space.

Ordinary text and attributes retain source ranges. Literal replacements such as decoded entities and normalized line endings can use static strings; genuinely constructed values use owned strings. Raw source ranges and decoded semantic values are separate: escapes, entities, container prefixes, and code normalization prevent treating every value as one verbatim slice.

Structured events already carry detail values separately: link and image destinations/titles, code content/language, and definition labels/destinations/titles each use `Text`. When a detail is `Text::Source`, its byte range identifies that value independently of the enclosing event span; for example, a code span retains both its delimiters in the event span and its trimmed content range in the value. Reference-link detail ranges can point to a definition elsewhere in the buffer rather than inside the link event span. Decoded or constructed details are not guaranteed to retain a separate raw range for each field or a character-by-character source map. The existing offsets are reused without adding per-event metadata storage.

Compact inline parsing extends escaped punctuation through the following ordinary source run. For `\*stars`, the visible `*stars` is one source-backed text event whose range starts after the escape backslash. The scanner stops before syntax candidates, including another escape, and does not concatenate decoded entities or disjoint source fragments into an allocated string. Delimiter and bracket candidates remain distinct until resolution. Text chunk boundaries and the corresponding number of AST text nodes are not canonical: consumers should use semantic content and source ranges rather than rely on one event per tokenizer fragment. Incremental AST construction still consumes one semantic event at a time within its caller-supplied budget.

The AST uses a contiguous arena with stable node indices and parent/child/sibling links, rather than a separately allocated child vector for every node. The builder tracks the last child of its active container with one cursor: opening a child resets the cursor, and closing that child restores it as its parent’s latest child. Finished nodes retain only parent, first-child, and next-sibling links. Editing a source-backed value materializes that value into an owned string. The source remains immutable while dependent ranges exist. Replacing or editing the backing source requires reparsing or materializing dependent values first; integer offsets alone do not track insertions and deletions. A rope, piece table, incremental reparsing system, and shared ownership runtime are not prerequisites for this design.

## Preparation and bounded inline work

Pull-based event delivery is distinct from accepting input in chunks. `EventParser` accepts a complete UTF-8 buffer; `IncrementalParser` accepts pieces and is described under [Incremental input](#incremental-input). CommonMark forward references and list tightness require preparation before some semantic events can be yielded. The block pass records source ranges, container relationships, and definitions. Inline parsing is deferred until its containing block is requested. Compact event delivery moves through explicit `Start`, `Body`, `DocumentEnd`, and `Done` phases. Infrequent document boundaries stay outside the hot body loop. After the optional final separator, it closes the document without walking the finished block tree again; final code/HTML separator rules are derived from the final block rather than maintained during leaf delivery. Temporary delimiter and bracket state is bounded by the active inline region, and its storage is reused across regions.

The block pass uses a compact arena of block relationships and source fragments for paragraphs, headings, lists, quotes, rules, code, all HTML block forms, tables, footnotes, math, and frontmatter. It does not emit grammar enter/exit tokens or retain per-line event edits. Base construct switches and optional extensions select individual syntax handlers within this same pass.

Definitions retain authored order and source-backed labels, destinations, and titles where possible. Separate sorted indexes select the first reference or footnote definition for each normalized raw identifier; duplicate authored definitions remain available to AST consumers. Forward references resolve because indexes are finished before event delivery. Definition lookahead traverses content fragments lazily, stopping at block interruptions, and commits a destination-only definition if an invalid title attempt started on a later line.

One inline scanner records candidates for text, escapes, entities, code, math, HTML, links, footnotes, tasks, and emphasis/strikethrough. It resolves brackets and delimiters within the active block, then delivers semantic events from reusable candidate storage. Literal web/email recognition handles Unicode domains, punctuation, balanced paths, and explicit-link precedence without a second tokenizer. Construct switches apply both to visible inline syntax and to decoding strings in destinations, titles, and code-fence info.

Ordinary multiline regions scan their original source fragments directly, without copying a stripped paragraph into a new string. A construct that needs lookahead across discontinuous fragments requests a reusable logical view with a source-segment map, then uses the same scanner. Normalization handles removed container prefixes, multiline code padding, and multiline resource/title whitespace. Public spans always refer to the original document. A value contained in one source segment stays source-backed; only constructed or normalized values need owned storage.

Markdown serialization traverses the document arena directly. Authored reference, footnote, and fence details are inferred lazily from source provenance, with sparse overrides for edits. There is no separate AST compiler or grammar compatibility producer.

## Incremental input

`IncrementalParser` accumulates pushed text and prepares only complete lines; a trailing carriage return waits for a possible line feed. Block preparation can start at any line recorded as a clean document-level boundary: a line whose preparation used no state from earlier lines (no open definition or table context) and which no earlier paragraph classified through table lookahead. Every block before such a line is closed. Preparation from that line, seeded with owned copies of the earlier definitions, reproduces the one-shot block structure from there on with the same byte offsets.

Each preparation records an outline of document-level blocks with their line, end and cleanliness, and whether the complete lines ended closed: a document-level blank line, or a clean ATX heading, thematic break or closed fence. Blocks before the last clean block, or all blocks when the outline is closed, are delivered through `EventParser`. Separators after a delivered block are held until the following block is delivered, because whether a list is followed by a separator depends on the kind of the next block. The parser then restarts at the latest delivered clean block and skips the blocks it has already delivered. Owned copies of the definitions before the new restart line are appended to a seed whose ordered index grows by insertion, and later preparations borrow the seed rather than copying or re-sorting it, so definitions far behind the restart point cost nothing per line. Only a first line that opens valid frontmatter (a valid fence and information string) holds back output until the frontmatter closes.

Definition lookup accepts a limit, which incremental delivery sets to the end of each document-level block before pulling its events. References therefore resolve against earlier definitions only, independent of how the input was divided. The repository records the requirements in `design/incremental-parsing.md`.

## Allocation and scanning budget

The block scanner’s line state uses 64-byte stack alignment to avoid the layout-sensitive stalls observed in arm64 preparation benchmarks. This pads each local line state from 48 to 64 bytes on the measured target; it does not enlarge parser objects or heap arenas. Paragraph tightness is derived from the paragraph’s immediate item/list parents when events are delivered. The block pass stores tightness only on the list, without copying it to every paragraph or walking finished lists to rewrite paragraph flags. Literal-link prefixes use small internal tags instead of fat string pointers, keeping every reusable inline record smaller. Block event delivery computes a closing source endpoint once and permits the compiler to inline the table-specific endpoint lookup. These optimizations preserve the public event representation.

The convenience HTML API initially reserves the source byte length, capped at 4 KiB, for its output string, then grows normally if needed. This skips early output growth steps while bounding speculative capacity when whitespace, definitions, or hidden constructs produce little or no HTML. A full-source reservation regressed the long-prose benchmark and was rejected. Callers that need their own capacity policy can pull events into a caller-owned writer with `render_events`.

HTML escaping uses one scanner and replacement writer for direct events and captures. It scans for the first replacement before entering its replacement loop. Unescaped spans use one ordinary write. Escaped spans select the active heading capture, footnote capture, or caller's writer once, then write unchanged runs and replacements directly without temporary strings or empty intervening writes. The renderer retains the last successfully written byte if a writer fails. GFM tag filtering emits unchanged source runs and escaped tag openers directly through that writer, including heading and footnote captures. ASCII-insensitive tag-name comparisons need no lowercase strings or intermediate filtered-HTML buffer. NUL replacement, raw-HTML handling, source ranges, and the shared parser/AST event contract are unchanged.

The target is no heap allocation merely to yield a start/end event or ordinary source text. Entity candidates store either a named-reference table index or a numeric Unicode scalar, without a separate vector of decoded values. Named values use static strings; numeric values become owned strings only when pulled. String decoding uses the same bounded character-reference scanner and retains unchanged source ranges.

Reference resolution reuses an ASCII normalization buffer and stores definition indices in candidates. Definition indexes are normalized and sorted once. Ordinary destinations and titles remain source ranges; constructed reference values still require cloning when opening links, and Unicode case normalization can allocate intermediate strings. Reference-free and extension-free regions use specializations of the same scanner to omit unused work. Logical-view and source-mapping buffers are allocated only when needed and reused across regions; no grammar workspace is boxed or promoted.

Parser nesting, delimiters, references, block records, AST nodes, and output need storage. Reuse vectors and caller-owned output, grow arenas geometrically, and avoid per-event boxes, queues, trait objects, or reference counts. A strict heap-free mode would need caller-provided capacity and explicit capacity exhaustion; it is not implied by this API.

Scan ordinary byte runs before invoking syntax handlers. Use byte-class tables for syntax candidates, indexed delimiter/bracket resolution, and batched source-to-output copies. SIMD is useful for finding candidate bytes and escaping HTML, not for replacing Markdown semantics. Prefer a small, portable scanner with a measured SIMD implementation and a tested scalar fallback. Keep architecture-specific unsafe code isolated, document its bounds and CPU requirements, and compare scalar and vector results at every boundary and alignment.

## Output compatibility

The default HTML API and arena traversal share one semantic interpretation. Opt-in heading IDs are generated from normalized semantic text, so multiline code and math spans no longer contribute whitespace that has already been removed from their visible value. For example, `` `\nb `\n--- `` produces `id="b"`, where the previous mdast-based heading capture produced `id="-b-"`.

Semantic end tags close their corresponding starts. Container indentation is removed from inline content, while code blocks retain any virtual spaces left by a partially consumed tab. Mixed ordered/unordered lists following unclosed fences remain siblings, and empty unclosed code-fence bodies remain empty. Escaped emphasis delimiters are classified from their authored neighbors. Separators between sibling blocks retain authored CR/LF/CRLF; separators inserted before container closing tags use the document’s first line-ending style.

Definitions remain invisible to HTML, retain their authored spans excluding the terminating line ending, and preserve duplicate order. Labels normalize Markdown whitespace and Unicode case without decoding escapes or entities. Source provenance remains available separately from normalized semantic identifiers.

Consolidation removes configuration-dependent grammar differences. Multiline labels inside quotes resolve without treating stripped `>` prefixes as identifier text, and table recognition is independent of the header or row's opening byte. Partial underscore runs use the same CommonMark delimiter rules with extensions enabled. Markdown serialization also consistently escapes text at boundaries that formerly depended on grammar token chunking. Regression fixtures cover these corrections; the saved comparison corpus retains byte-identical output.

Tables retain alignment, header flags, and authored cell spans independently of trimmed inline content. Missing cells have empty spans and surplus body cells are ignored. Escaped pipes are handled before inline resolution, including inside code spans. Retained HTML blank lines stop at their enclosing container boundary. Frontmatter requires a matching closing fence and is recognized only at the start of the document.

## Validation and performance acceptance

Validate parser → HTML and parser → AST → events → HTML against each other, the existing output corpus, CommonMark, GFM, extensions, edited documents, and malformed input. Test bounded AST construction, early drop, deeply nested input, source detachment, and writer failures. Preserve existing public APIs until their replacements cover the supported behavior.

Measure end-to-end rendering, event consumption, AST construction/traversal, allocation counts, and adversarial scaling separately. Use the pinned six-engine harness in `benches/comparison/`, report exact output agreement alongside timings, and retain raw samples and source revisions. Compare cold allocations and reused workspace/output separately. Do not claim the Inkmark target is met based on a subset fast path, different extension behavior, or a scanner-only microbenchmark.

The algorithmic references are pulldown-cmark's [two-pass pull design](https://pulldown-cmark.github.io/pulldown-cmark/dev/index.html) and [candidate-byte SIMD scanning](https://pulldown-cmark.github.io/pulldown-cmark/dev/performance.html). Any copied source must retain its license and identify its exact upstream revision; using these techniques does not require selecting another Markdown backend.

## Initial public API

`EventParser::new(source, options)` prepares source-backed block records and definition indexes with the single parser. `render_events(source, events, compile_options, output)` pulls any producer through `EventHtmlRenderer`. `Document::parse(source_buffer, parse_options)` moves a caller-owned buffer into a source-backed AST, and `Document::events()` traverses it without allocating a traversal stack or cloning edited strings.

## Infallible construction and HTML conversion

`EventParser::new` now returns `EventParser` directly, and `to_html_with_options` returns `String`. Remove `?`, `.unwrap()` or `.expect(...)` from those two calls. This is a breaking API change that removes error wrappers retained after MDX removal.

```rust
use base_markdown::{EventParser, Options, to_html_with_options};

let options = Options::default();
let html: String = to_html_with_options("# Heading", &options);
let parser = EventParser::new("# Heading", &options.parse);
```

The event iterator still yields `Result<Event, StreamError>` for the shared consumer contract. `render_events`, incremental AST construction, document parsing/editing and Markdown serialization retain errors for custom producers, invalid streams or source ranges, output failures and invalid serialization options. These calls still use normal error handling.

For bounded construction, retain an `AstBuilder` and call `consume(&mut events, event_budget)` until it returns `BuildProgress::Complete`. `Pending` means the budget was exhausted without probing for another event. Drop the parser before calling `finish(source_buffer)` to move the original buffer into the resulting document. `text_mut(node_id)` materializes only the edited value; node metadata can also be assigned through checked `set_kind`. Publicly supplied event streams and source ranges are checked. Direct Markdown-to-HTML rendering creates its parser from the renderer’s own source and uses that producer’s range and tag invariants instead of repeating range validation for every event; it retains the same renderer, balance checks, and safe string resolution. The checked parser → events → HTML and arena paths are compared with this internal path in regression testing. Fixed HTML tags and adjacent attribute delimiters are emitted together from static strings; dynamic values retain the same escaping and URI sanitization. This reduces logical `fmt::Write` calls without constructing temporary strings. Write-call boundaries are an implementation detail; writer errors still propagate.

## Borrowed implementation techniques

The block/container scanners and algorithms are adapted from `pulldown-cmark` 0.13.4, revision `38e4d08f14ec4bd9783270e9623db7681ebed968`, specifically `firstpass.rs` and `scanners.rs`. The copied scanners retain their MIT notice. Unlike that revision's first pass, this block pass records only source fragments for paragraphs and headings; it does not eagerly scan inline candidates throughout the document. It recognizes CR, LF, and CRLF, distinguishes Markdown indentation from VT/FF, and caches failed thematic-break lookahead using absolute offsets.

Block opening and paragraph interruption dispatch on the first content byte before invoking the applicable scanner. Blank continuations finish paragraphs before block lookahead. Ordinary continuation text avoids unrelated heading, fence, rule, and list probes; only list candidates need the aligned list-lookahead cursor. Disabled literal autolinks and strikethrough omit their cleanup. Empty inline delimiter scopes return without initializing the emphasis resolver; outer scopes and the next nonempty scope retain their ordinary resolution rules. These changes add no parser state or allocations.

Container-prefix scanning skips the document root, which is always open and consumes no source bytes. Block opening retains the indentation already scanned while recognizing list and quote prefixes, rather than restoring the cursor and scanning it again. The pre-indentation source offset and partial-tab state remain available for definitions, HTML, code, and table classification. These changes add no cache, allocation, or public event representation.

The inline producer indexes backtick runs by length once per source fragment, resolves emphasis using an opener stack and failed-search bounds, and invalidates nested link openers with a monotone boundary. Ordinary text, link destinations, code spans, and unchanged attributes remain source ranges; named character references use static decoded values. It prepares only the active inline block before yielding resolved events.

Candidate-byte scanning uses a two-nibble lookup table with AArch64 NEON when the target enables it, plus a portable scalar path. Inline text and HTML escaping share the scanner. Differential tests cover every input byte at every position around vector boundaries and multiple alignments; no vector load crosses the input slice. The default target remains portable and does not require runtime CPU detection.

NEON scanning retains the detection-only loop for chunks without candidates. After detecting a candidate, it selects each matching lane's index and takes their minimum, avoiding a second scalar scan of that chunk. Short tails retain the scalar lookup. URI sanitization separately uses a static byte-class table to scan safe runs, while preserving protocol checks, percent-triplet acceptance, and byte-wise encoding of Unicode. Neither change allocates or alters the semantic event contract.
