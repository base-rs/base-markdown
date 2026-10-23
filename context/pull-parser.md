# Pull Parser Architecture

The parser and document traversal produce the same pull-based semantic event stream. HTML rendering and AST construction consume that stream without first collecting it. This document specifies the implementation being introduced; the existing mdast API remains available during migration.

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

The AST uses a contiguous arena with stable node indices and parent/child/sibling links, rather than a separately allocated child vector for every node. Editing a source-backed value materializes that value into an owned string. The source remains immutable while dependent ranges exist. Replacing or editing the backing source requires reparsing or materializing dependent values first; integer offsets alone do not track insertions and deletions. A rope, piece table, incremental reparsing system, and shared ownership runtime are not prerequisites for this design.

## Preparation and bounded inline work

Pull-based event delivery is distinct from accepting input in chunks. The initial implementation accepts a complete UTF-8 buffer. CommonMark forward references and list tightness require preparation before some semantic events can be yielded. The block pass records source ranges, container relationships, and definitions. Inline parsing is deferred until its containing block is requested. Temporary delimiter and bracket state is bounded by the active inline region, and its storage is reused across regions.

The ordinary CommonMark block pass uses a compact arena of block relationships and source fragments. It recognizes paragraphs, headings, lists, quotes, rules, and code directly, without producing grammar enter/exit tokens or per-line edit vectors. Inline resolution reuses one region's tokenizer storage and never splices inline tokens into the document-wide block buffer. The default HTML API uses the shared event writer; the old HTML token compiler has been removed.

Migration is incomplete. Documents containing reference definitions or raw block HTML, extension configurations, and individually disabled constructs currently select the grammar compatibility producer. Ordinary inline text, escapes, entities, emphasis, same-line code spans, and inline links use compact candidates in reusable storage. Multiline code/resources, inline HTML, and inline-code language metadata still select the grammar engine for their containing region. These paths retain their behavior and must remain separately identified in performance measurements. The legacy mdast API also retains its own parser until semantic events cover its representation-specific metadata.

## Allocation and scanning budget

The target is no heap allocation merely to yield a start/end event or ordinary source text. The compatibility producer still normalizes reference labels and clones constructed reference destinations when opening links; these allocations remain a measured migration cost. Parser nesting, delimiters, references, block records, AST nodes, and output need storage. Reuse vectors and caller-owned output, grow arenas geometrically, and avoid per-event boxes, trait objects, cloned strings, queues, or reference counts. A strict heap-free mode would need caller-provided capacity and explicit capacity exhaustion; it is not implied by this API.

Scan ordinary byte runs before invoking syntax handlers. Use byte-class tables for syntax candidates, indexed delimiter/bracket resolution, and batched source-to-output copies. SIMD is useful for finding candidate bytes and escaping HTML, not for replacing Markdown semantics. Prefer a small, portable scanner with a measured SIMD implementation and a tested scalar fallback. Keep architecture-specific unsafe code isolated, document its bounds and CPU requirements, and compare scalar and vector results at every boundary and alignment.

## Output compatibility

The default HTML API and arena traversal share one semantic interpretation. Opt-in heading IDs are generated from normalized semantic text, so multiline code and math spans no longer contribute whitespace that has already been removed from their visible value. For example, `` `\nb `\n--- `` produces `id="b"`, where the previous mdast-based heading capture produced `id="-b-"`. The legacy mdast API remains available with its existing representation.

Semantic end tags close their corresponding starts. The compact block producer makes mixed ordered/unordered lists following unclosed fences siblings, consumes container indentation tabs, and leaves empty unclosed code-fence bodies empty. These correct legacy hierarchy, tab, and synthetic-newline anomalies; the compatibility producer retains its old interpretation. Escaped emphasis delimiters are classified from their authored neighbors, correcting `\\**x**` to literal `*`, emphasized `x`, and literal `*`. Separators between sibling blocks retain their authored CR/LF/CRLF style. Separators inserted before closing container tags use the document’s first line-ending style; a mixed-ending input such as `x\n\n- # a\r\n ` therefore uses LF between `</h1>` and `</li>`, where the legacy grammar retained CRLF. This is an intentional formatting normalization; rendered structure is unchanged. Failed frontmatter lookahead also restores correct line positions instead of producing orphan quote closures. Reference-label normalization now preserves a collapsed space after the first word; labels such as `a b` and `ab` no longer collide.

## Validation and performance acceptance

Validate parser → HTML and parser → AST → events → HTML against each other, the existing output corpus, CommonMark, GFM, extensions, edited documents, and malformed input. Test bounded AST construction, early drop, deeply nested input, source detachment, and writer failures. Preserve existing public APIs until their replacements cover the supported behavior.

Measure end-to-end rendering, event consumption, AST construction/traversal, allocation counts, and adversarial scaling separately. Use the pinned six-engine harness in `benches/comparison/`, report exact output agreement alongside timings, and retain raw samples and source revisions. Compare cold allocations and reused workspace/output separately. Do not claim the Inkmark target is met based on a subset fast path, different extension behavior, or a scanner-only microbenchmark.

The algorithmic references are pulldown-cmark's [two-pass pull design](https://pulldown-cmark.github.io/pulldown-cmark/dev/index.html) and [candidate-byte SIMD scanning](https://pulldown-cmark.github.io/pulldown-cmark/dev/performance.html). Any copied source must retain its license and identify its exact upstream revision; using these techniques does not require selecting another Markdown backend.

## Initial public API

`EventParser::new(source, options)` prepares compact block records or selects the documented compatibility producer. `render_events(source, events, compile_options, output)` pulls any producer through `EventHtmlRenderer`. `Document::parse(source_buffer, parse_options)` moves a caller-owned buffer into a source-backed AST, and `Document::events()` traverses it without allocating a traversal stack or cloning edited strings.

For bounded construction, retain an `AstBuilder` and call `consume(&mut events, event_budget)` until it returns `BuildProgress::Complete`. `Pending` means the budget was exhausted without probing for another event. Drop the parser before calling `finish(source_buffer)` to move the original buffer into the resulting document. `text_mut(node_id)` materializes only the edited value; node metadata can also be assigned through checked `set_kind`. Publicly supplied event streams and source ranges are checked rather than trusted.

## Borrowed implementation techniques

The block/container scanners and algorithms are adapted from `pulldown-cmark` 0.13.4, revision `38e4d08f14ec4bd9783270e9623db7681ebed968`, specifically `firstpass.rs` and `scanners.rs`. The copied scanners retain their MIT notice. Unlike that revision's first pass, this block pass records only source fragments for paragraphs and headings; it does not eagerly scan inline candidates throughout the document. It recognizes CR, LF, and CRLF, distinguishes Markdown indentation from VT/FF, and caches failed thematic-break lookahead using absolute offsets.

The inline producer indexes backtick runs by length once per source fragment, resolves emphasis using an opener stack and failed-search bounds, and invalidates nested link openers with a monotone boundary. Ordinary text, link destinations, code spans, and unchanged attributes remain source ranges; named character references use static decoded values. It prepares only the active inline block before yielding resolved events.
