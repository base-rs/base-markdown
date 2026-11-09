# Normalize and serialize contract

This is what a `Document` holds and what `Document::normalize`, `Document::is_normalized`, `to_markdown` and HTML rendering guarantee. It is the one statement of those guarantees: the rustdoc of each function says how it does its part, and where the two seem to disagree, this contract is what is meant and the other text is a bug. Each clause has tests named after it in `tests/contract.rs`, `clause_<section>_<clause>_…`, and the known gaps, where the implementation falls short of a clause today, are listed in section 8, each with its issue.

Throughout, "the HTML" of a document is `Document::render_html` with the given `CompileOptions`, "written" is `to_markdown_with_options`, and "read back" is `Document::parse` with `ParseOptions::default()`, the syntax `normalize_markdown` reads.

## 1. The model

**1.1 Arena and trees.** A `Document` is an arena of nodes addressed by `NodeId`, scoped to that document. Node 0 is the root of the *attached* document, `Document::root`. Every other node with no parent is the root of a *detached* tree: what `detach` and `extract_children` take out, or what an edit left there. A detached tree can be inspected, rendered, written and attached again. Arena slots are never reclaimed while the document lives, so an ID stays valid across edits until a clause says otherwise (3.6). Each tree is self-contained for names: its references bind only to its own definitions (2.2).

**1.2 Content model.** One content model decides what each container can hold, `TagKind::holds` in `stream.rs`. The root, block quotes, items and footnote definitions hold blocks and separators; paragraphs, headings, table cells, emphasis, strong, strikethrough, links and images hold phrasing; a list holds items; a table holds heads and bodies, they hold rows, with separators between them, and a row holds cells. A root whose first child is phrasing is an *inline document* and holds only phrasing; one whose first child is a block holds no phrasing. Rules look at ancestors (`Reach`): no footnote call inside a link, and no soft or hard break inside a table cell, nor inside a heading of level 3 to 6, or any heading where `Constructs::heading_setext` is off, as only a setext heading spans lines. A heading level is 1 to 6, a label is one the parser can read, and each value has the form the parser stores (#257): no value holds NUL, which the parser stores as U+FFFD; a raw value, which is written as it is, inline code, a code block, HTML or frontmatter, holds no CR, which it stores as LF, while a decoded value, text, a destination, a title or an info string, may, as the writer encodes it as `&#xD;`; inline code holds no line ending, which it reads as a space, and may be empty, which normalizing removes; inline HTML is empty or one HTML construct the inline parser reads, with nothing after it, no blank line and no space or tab after a line ending, which a paragraph's continuation line loses (#256); an HTML block is one block the block parser reads, by its start and end conditions, with the indentation the parser keeps before one, up to three spaces, or one or two spaces then a tab and spaces, where those spaces and the tab, counted as one column, are at most three, or any where `Constructs::code_indented` is off, without its final line ending (#213), and with no blank line where its kind ends at one, though one of kinds 1–5 that no line closes may end in the blank lines it takes, which are its content; and frontmatter that is not empty ends in a line ending. A code block that is not empty and lacks a final line ending is valid, as the parser gives one for an unclosed fence or indented code at the end of the source, but anywhere other than the end of the document it is not normalized: normalizing adds the line ending (3.1). So `abc`, `<b> x` and `<div>\n\nb` are no HTML block, nor, under the default options, `    <div>`.

*The dialect.* A document records the `ParseOptions` it is valid under, `Document::parse_options`: those `Document::parse` read it with, those given to `AstBuilder::with_options`, or the defaults, for `AstBuilder::new` and a serialized document without them. A node is valid when some parse with them could produce it there, so these rules are read under them: a node of a construct that they turn off is invalid, such as a table without `Constructs::table`, a heading of level 3 to 6 without `Constructs::heading_atx`, a code block with an info string, or whose value is empty or starts or ends with a blank line, without `Constructs::code_fenced`, or an inline code language without `ParseOptions::inline_code_info`; inline HTML and an HTML block may have namespaced tag names only under `ParseOptions::html_tag_namespaces`; inline HTML that `Constructs::autolink` reads as an autolink, such as `<tel:123>`, is invalid where it is on, as it reads back as a link; and a block of kind 6 or 7 may hold blank lines before a line indented at least as far as the least indented line before it, and a final line ending, only under `ParseOptions::html_block_blank_lines`, which keeps them. A node's form is not always recorded, so the check is what some form allows: a link of its own is valid where a label start and `Constructs::label_end` are on, or where an autolink construct is, and a `[^` that no footnote definition binds starts a link as `[` does. `Document::set_parse_options` records other options when every node, attached or detached, is valid under them, and converts nothing. `insert_from` and `replace_heading_section` check each copy against this document's options. A renderer, `render_events` or `EventHtmlRenderer`, has no options, and takes a value that any options read. Every path that adds nodes checks these: `AstBuilder`, `EventHtmlRenderer`, `append_child`, `attach`, `set_kind`, `parse_children`, `insert_from`, `replace_heading_section` and deserialization. A violation of the content model or of `Reach` is `StreamError::InvalidNesting`, an invalid value `StreamError::InvalidValue`. So an invalid tree cannot be built.

**1.3 Valid is not normalized.** Shapes that every tree passes through while being built are valid: empty containers, empty or adjacent text, adjacent attention of the same kind, a table without a head or with rows of different widths, a link inside a link, unbound references (2.5) and copies of a binding (2.3). They are valid and not normalized: section 3 turns them into the shape that is written exactly.

**1.4 Values.** Parsed values are byte ranges into the source, which stays immutable; `text_mut` and other edits copy a value out before changing it. Every source line ending is stored as LF, and NUL as U+FFFD, so a value is copied out of a source only where it holds a CR or NUL. `text_mut` edits only text and soft breaks, whose values hold anything but NUL, which it cannot check; other values change through `set_kind`, which checks them. Spans are half-open byte ranges and say where a node came from, not where it would be written.

## 2. Names and bindings

**2.1 References are names.** A reference link or image stores its kind (full, collapsed or shortcut) and its label, `Target::Reference`, with no destination; a footnote call stores its identifier and label. Nothing stores a binding on a reference: where it leads is found when asked, `Document::destination`, `Document::bindings`, or the events, so it follows every edit of the definitions.

**2.2 The first definition binds.** In each tree (1.1), a label is bound by the first link reference definition of it in document order, matched as the parser matches labels. A footnote identifier is bound by the first footnote definition of it *to end*, which is the first in document order unless one is nested in another of the same identifier, when the nested one binds.

**2.3 Copies are accepted on input.** A later definition of a bound label or identifier is a *copy*. `Document::parse`, `AstBuilder` and deserialization accept copies, as Markdown does. A copy binds nothing and renders nothing.

**2.4 No edit adds a copy to the attached document.** An edit that would add to the attached document a definition or footnote definition of a label or identifier already defined there, or a subtree that defines one twice, is rejected with `StreamError::InvalidValue` before anything changes: `set_kind`, `append_child`, `attach`, `parse_children`. `insert_from` and `replace_heading_section` copy no link reference definitions and relabel or reject footnote definitions (6.3). Moving a definition within the attached document, with `attach`, is rejected with `InvalidValue` too when its label or identifier has a copy: the document's index keeps a label's definitions in document order, the first binding, so the move could reorder them and change which one binds. Detaching a definition unbinds it, so the only definition of a label can be detached and attached again; a copy, once detached, cannot. A detached tree has no index, so edits inside one are not checked for copies, which would take quadratic time (#226): it may hold copies, and attaching a tree that holds copies, or that defines what the attached document defines, is rejected.

**2.5 Unbound references are valid.** A reference or footnote call that nothing in its tree binds is valid but not normalized. Its events, HTML, `text_content`, headings and heading ids give it as the text it reads back as. `normalize` makes it that text (3.1).

## 3. Normalize

**3.1 What it does, in order.** `Document::normalize` normalizes every tree, attached and detached, in time linear in the size of the arena. For each tree, in this order:

1. Copies (2.3) are removed, each place they leave given the separators the parser puts there.
2. References and footnote calls that the tree does not bind become their text (2.5), except one that is itself the root of a detached tree, which has no parent to hold its text and stays a reference. This is skipped when the document is settled (4.1).
3. A link inside another link's text is unwrapped, unless it is an autolink that reads back as one.
4. Unwritable empties are removed: inline HTML and inline code without a value, lists without items and tables without a cell.
5. Every child list is normalized: empty text and empty attention are removed, adjacent text is joined, adjacent inline code is joined when the second has no language, adjacent attention of the same kind is merged, strikethrough inside strikethrough is unwrapped, whitespace and breaks move out of the edges of attention, and breaks at the edges of a block are removed.
6. Empty paragraphs are removed.
7. Items of tight lists get blocks that can be written tight: a list after a paragraph loses its leading empty items, an ordered one starts at 1, two paragraphs in a row are joined with a soft break, and the list is made loose where nothing else works. A loose list whose only item has fewer than two blocks is made tight.
8. Tables get the parser's shape: one head row, rows as wide as the widest, header cells and alignment as the head's.
9. Task items whose checkbox would be written before nothing stop being task items, and the rest get the space the parser keeps before their text.
10. The end of the document gets the separators the parser puts there.

Then a code block that is not empty and has no final line ending gets one unless it is the last node of the document, and labels get a shape that reads back wherever they are written, a label too long for a reference becoming a resource. No other value is rewritten: each already has the form the parser stores (1.2), so normalizing never changes a character of one. The rustdoc of `Document::normalize` gives each rule exactly.

**3.2 Idempotence.** After `normalize`, `is_normalized` holds, and normalizing again changes nothing: not the tree, not an ID, not a value. `is_normalized` holds exactly when `normalize` would change nothing, and takes linear time.

**3.3 Round trip.** For a normalized document `d` whose root holds blocks, with `MarkdownOptions` that it writes without an error, let `r` be `d` written, read back and normalized. Then `r` is `d` (compared without spans, IDs, separators, label case and whitespace, the line ending the parser keeps after a hard break or a task checkbox, and the final line ending of a last code block that has none), `r` has the HTML of `d` but for the line ending at the very end and those after hard breaks, and writing `r` gives exactly the Markdown that writing `d` gave. With other `ParseOptions` the writer does not know the options (5.2), and the round trip is not promised.

The options change what is written, not what it reads back as, with one exception: `LineWrapping::Unwrap` writes each soft break as one space, but the line ending of a hard break, and each run of line endings, spaces and tabs in a text value that holds a line ending as one space, so `r` is `d` with that change, but for a shortcut or collapsed reference whose text no longer matches its label, which is written full, and has its HTML. `normalize_markdown("a\nb\n")` is `"a b\n"`, and a heading that held a soft break is written in ATX form. With `rule: '-'`, a rule that a `---` would not write is written with `*`: one on the line after a paragraph or definition, which `---` would underline as a setext heading, and one starting the document that a later line would close as frontmatter. One option breaks the round trip today: `fences: false` writes `<!---->` between a list and indented code after it, which reads back as an HTML block (#157).

An inline document (1.2) is written as a paragraph's content, so it reads back as a document whose root holds one paragraph; that paragraph's content is what 3.3 promises of the root's, though not yet for every text (#223), and `attach` can still make an inline document from one that held blocks (#231).

**3.4 HTML.** Normalizing changes the HTML only as follows.

- *As parsed.* For a document as `parse` or the parser's events built it, the HTML is unchanged but for attention markup: merging adjacent attention of the same kind, unwrapping strikethrough in strikethrough, moving whitespace and breaks out of attention's edges and removing attention left empty change where `<em>`, `<strong>` and `<del>` tags open and close, never the text or its order. A parsed document has no other shape that section 3.1 rewrites, except copies and the gaps of section 8.
- *Removing copies.* Removing copies keeps the HTML, with five exceptions:
  1. A loose list whose only item is left with fewer than two blocks becomes tight, so `[x]: /1\n\n- a\n\n  [x]: /2\n` renders `<li>a</li>` where it rendered `<li>\n<p>a</p>\n</li>`.
  2. The renderer counts footnote calls inside a footnote definition that is a copy, though it shows nothing of the copy, so once it is removed the footnote numbers, anchor ids, the order of entries, back references to calls not in the output and the entry of a footnote called only there can change (#225).
  3. An item whose only block was a copy is left empty, and when it starts a list directly after a paragraph in an item of a tight list it is removed (rule 7), as `- p\n  -` reads back as a heading: `[x]: /1\n\n- p\n  - [x]: /2\n` loses the inner list.
  4. Blocks that a removed copy leaves in a row in an item of a tight list, which cannot be written as they are, get rule 7: two paragraphs are joined with a soft break, an ordered list directly after a paragraph starts at 1, as `x[^a]\n\n[^a]: z\n\n- p\n  [^a]: y\n  2. q\n` loses `start="2"`, and otherwise the list is made loose. The pairs that need it, which `needs_blank_line` in the writer decides for both, are two block quotes or two tables, which would read back as one; a definition, a heading that only a setext heading can write, or an HTML block of kind 7 after a paragraph, which none of them can interrupt, and an HTML block of kind 7 after a definition, for the same reason, so an item of `- p\n- q\n` holding a paragraph `a` and a definition `[a]: /x` is written `- a\n\n  [a]: /x\n\n- q\n` (#232); a paragraph, definition or such a heading after a table, whose rows it would be read as; any block after an HTML block of kind 6 or 7, which ends only at a blank line, or of kinds 1–5 whose last line does not hold its end condition, so one holding `<b>` and an empty heading is written `- <b>\n\n  #\n\n- q\n`; and a block after a nested list or footnote definition that it would continue lazily. Any other pair is written on the next line, so the list stays tight. An HTML block of kinds 1–5 that no line closes takes every line after it in its container whatever separates them, so one followed by a block, which only an edit gives, does not read back: the writer writes it without an error, and the block reads back as more of the HTML block (#191).
  5. Where a removed copy after a list, or at the end of the document, leaves the list next to what follows it, or at the end, the separators the parser puts there are given, so the round trip (3.3) holds, and the HTML gains or loses the line ending they render: `- [c]: /6\n\n[c]: /6\n` renders `<ul>\n<li></li>\n</ul>` and its normalized form, `- [c]: /6\n`, a line ending after it.
- *Edited trees.* The other rules of section 3.1 rewrite shapes that only an edit makes, so that they can be written, and change the HTML where those shapes render differently from what can be written: unbound references and unwrapped links render as text, removed empties as nothing, a table in the parser's shape, and the tight-item rules join paragraphs, renumber lists and loosen lists as in exception 4.

**3.5 Copies dismantled.** A copy is removed with everything it holds, but for definitions in a copied footnote definition that bind their own labels, which take its place. Each other node a copy held becomes a detached root of empty text, which binds, refers to and holds nothing, so a removed copy is never a tree to normalize or look names up in again.

**3.6 IDs.** The IDs of removed, joined, merged and unwrapped nodes no longer refer to nodes in the tree: their slots remain, detached, with unspecified contents. Other IDs stay valid, and the rustdoc of `Document::normalize` lists which nodes keep their IDs and which are new.

## 4. Settled

**4.1 Meaning.** A document is *settled* when every reference and footnote call in every tree is known to be bound. Settled is not normalized: a parsed document is settled and usually not normalized, as its text is split at escapes, and a normalized document is always settled. It is private, and changes only how much work is done: while it holds, `normalize` and `is_normalized` look nothing up for references, and `insert_from` does not walk the tree for unbound footnote calls.

**4.2 What keeps it.** `Document::parse` sets it, `AstBuilder::finish` sets it when its stream gave every reference resolved and bound and every footnote call its definition, and `normalize` sets it. `insert_from` keeps it, as what it copies is bound or text and its copied definitions take no identifier a call names; an edit that fails keeps it, as it changes nothing; `text_mut`, `set_syntax` and cloning keep it, as they change no name. Every other successful edit clears it: `set_kind`, `append_child`, `detach`, `attach`, `extract_children`, `parse_children` and `replace_heading_section`. A deserialized document is not settled.

## 5. Output

**5.1 Writing a normalized document.** `to_markdown` writes LF line endings, ends non-empty block output with one, and is deterministic. For a normalized document it is correct and stable, as section 3.3 states.

**5.2 Writing other documents.** A document that is not normalized is written without panicking, but possibly incorrectly or unstably: a shape that section 3.1 would rewrite is written as it is, or as the nearest Markdown, which may read back differently. Writing a parsed document without normalizing it usually gives the same output. For example, `insert_from` of a paragraph after the paragraph of an item of a tight list keeps two paragraphs in the tree, which render `<li>a\nPara</li>` as the parser renders them, and writing that tree before normalizing gives `- a\n\n  Para\n`, a loose item; normalizing joins them (rule 7), and the HTML stays `<li>a\nPara</li>`. The writer does not know the parse options, so output read back with other options than the default may differ.

**5.3 Writing errors.** Writing returns an error, never a panic, for invalid options or syntax metadata, content nested more than 128 levels deep (`nesting-too-deep`), and an ordered item whose number would have more than 9 digits (`list-number-too-long`).

**5.4 HTML.** `Document::render_html` renders the document's events, `Document::events`, which give each bound reference as `Target::Resolved` to its definition, and unbound ones as their text (2.5). So the HTML of a parsed document is the parser's: `render_html` of `Document::parse(s, &options.parse)` equals `to_html_with_options(s, &options)`. `render_events` takes resolved targets only and rejects a `Target::Reference` with `StreamError::InvalidValue`. `render_node` renders one subtree in fragment mode, with the document's bindings, and errors only for an invalid ID or writer.

**5.5 `normalize_markdown`.** `normalize_markdown(s)` is `s` parsed with the default options, normalized and written with `LineWrapping::Unwrap`, so it reads back as the normalized tree of `s` with each soft break and line ending in text a space (3.3).

## 6. Edits

**6.1 Atomic.** An edit that returns an error changes nothing: not the tree, the index, a value, an ID or whether the document is settled. Each edit plans and checks first, then changes.

**6.2 Errors.** Edits use these kinds:

| Error | When |
| --- | --- |
| `InvalidRange` | An ID that is not in the arena, as a node or a parent, a source range outside the source, a position that is not a child of the parent (`attach`'s `before`, `insert_from`'s `after`), a node outside the source's attached document (`insert_from`), `text_mut` of a node that is not text or a soft break, syntax for an incompatible node. |
| `Unbalanced` | A leaf parent, an end event appended, a cycle, moving or detaching the root. |
| `InvalidNesting` | A node that the content model or `Reach` does not allow there (1.2), such as a break in a heading of level 3 to 6, a change of kind that `set_kind` cannot make in place, such as a heading level whose headings cannot hold the breaks it holds, a fragment that does not parse into what its parent holds, options under which `set_parse_options` would leave such a node. |
| `InvalidValue` | An invalid value (1.2), such as a node of a construct that the document's options turn off, a heading level outside 1 to 6, a label the parser cannot read, NUL anywhere, a CR in a raw value, a line ending in inline code, inline HTML that is not empty or one HTML construct, or an HTML block that is not one block construct; an edit that would add a copy to the attached document, or move a definition whose label has a copy (2.4); a footnote clash under `FootnotePolicy::Reject`; a fresh identifier or label too long to be one. |

**6.3 Copying between documents.** `insert_from` and `replace_heading_section` copy so that each copy renders as its original does in its own document:

- A bound reference becomes a resource to where it leads there, and an unbound one its text, so link reference definitions are never copied.
- Each footnote call brings the footnote definition it calls, once, after the copies, with the calls in it bringing theirs; footnote definitions nothing copied calls are left out.
- A copied footnote definition whose identifier is taken here, bound or named by a call that nothing binds, gets a fresh identifier under `FootnotePolicy::Relabel`, the default, with its calls rewritten, and is rejected under `FootnotePolicy::Reject`; so a call here keeps calling what it called, or nothing.
- `InsertOptions::heading_shift` is added to each copied heading's level, and a level outside 1 to 6 is rejected, not clamped.
- Copies into a container of blocks get the separators the parser writes there.

Same-document moves keep references as names, which bind as the document now does.

## 7. Costs

**7.1 Linear.** Parsing, building, `normalize`, `is_normalized`, writing and rendering take time linear in their input, as do `parse_children`, `insert_from` and `replace_heading_section` in what they parse, copy and replace. Appending in document order takes amortized constant time per node. A name lookup in the attached document takes constant time usually, and at most logarithmic time in the number of labels.

**7.2 Known quadratic cases.**

- Lookups in a detached tree build a table of that tree each time, so a per-node call on each node of a large detached tree, `text_content`, `render_node` or `to_markdown_node`, and many `parse_children` or `insert_from` with footnotes into one detached tree, take quadratic time (#226).
- An edit that has to know whether a link or table cell is above its parent walks up through every phrasing ancestor, so many edits deep in one phrasing nest take quadratic time: `append_child` of a footnote call or break, `attach` and `parse_children` (#233).
- `insert_from` of a footnote definition into a document that is not settled walks the tree once for unbound calls, so many such inserts into a document edited since it was parsed or normalized take quadratic time.

## 8. Known gaps

Every open issue, classified: those where the implementation breaks a clause, with the clause; parser conformance gaps, where the parser differs from the specification or reference implementations; and enhancements, which no clause promises.

| Issue | Clause | Gap |
| --- | --- | --- |
| #89 | 3.3 | Text after literal-autolink-like text writes unstably. Its repros round trip on main, as `tests/contract.rs` checks, so it may be fixed. |
| #99 | 3.3 | Encoded whitespace before attention stops it opening when written. Its repro round trips on main, so it may be fixed. |
| #108 | 3.3 | Lazy table-like lines in an item or quote become a table when written. Its repros round trip on main, so it may be fixed. |
| #110 | 3.3 | Nested attention: a switched inner marker lets a closing run glue onto a sibling's opener; the `tree-*` fuzz corpora fail on it. |
| #111 | 3.3 | A `**` that can close may close an enclosing `*` emphasis. No repro yet. |
| #149 | 3.3 | Hand-built tight lists whose items record spread read back loose. |
| #157 | 3.3 | Older list separator cases: a nested list ending in a heading, spread from span gaps, and `fences: false`, which writes `<!---->` before indented code after any list: `- a\n\n```\nx\n```\n` is written `- a\n\n<!---->\n\n    x\n`. |
| #168 | 3.3 | Tight-item block pairs that still read back loose, and soft-break escapes. |
| #177 | 3.3 | Block quotes in items: a tight list reads back loose, and two quotes merge. |
| #181 | 3.3 | A paragraph after a quote ending in an empty item leaves the outer item. |
| #183 | 3.3 | With `fences: false`, a tight item of a paragraph and indented code is written as a lazy continuation. Its inline HTML shapes, of whitespace alone or with a leading space, are rejected since #193. |
| #191 | 3.3 | An HTML block of kinds 1–5 that no line closes, followed by another block, which only an edit gives, does not read back. A list ending in one is written with the next block on the following line, as the block would take a blank line there (#257). That loses the only blank line of a loose list whose one item holds such a list and then another block, which only an edit gives, as `- - <pre>\n  x` set loose, so it reads back tight. |
| #193 | 3.3 | Inline HTML shapes that cannot be written. A paragraph or setext heading whose first line would start an HTML block with inline HTML, such as `<div>`, or `<b>` alone on its line, can be written only on the line after a definition, indented, and anywhere else reads back as an HTML block: one built by hand, or one left first in its container by a removed copy, `[x]: /1\n\n- [x]: /2\n      <div>\n`, or `> [x]: /1\n\n> [x]: /2\n> <b>\n`, where the parser reads `<b>` as text after a definition (#249). `<b>` before a soft break in the removed-copy shape, `[x]: /1\n\n- [x]: /2\n      <b>\n      c\n`, is written `- <b>&#xA;c`, which keeps it inline but makes the soft break text. |
| #196 | 3.3 | Inline HTML with a line ending in a setext heading can start a line with a block marker. |
| #199 | 3.3, 3.4 | A task item whose text starts with a hard break or line ending: normalizing adds a space before the break, which changes the parsed HTML, and the output reads back differently. |
| #200 | 3.3 | A leading definition in a task-like item does not read back; the checkbox is written before the definition, as in `1. [a]:\n  --\n[x] [y]\n`, whose task paragraph the parser reads after the definition since #249. |
| #205 | 3.3 | An escape after literal-like text in link text is dropped. |
| #206 | 3.3 | With `resource_link`, an autolink in link text is written as a nested link. |
| #207 | 3.3 | With `LineWrapping::Unwrap`, a literal before a soft and a hard break writes unstably. |
| #208 | 1.2, 3.3 | A definition label starting with `^` reads back as a footnote definition; the parser makes one from `[a]: /u\n    [^b]: #\n`. |
| #209 | 3.3 | Labels holding `\|` in table cells do not read back. |
| #210 | 3.3 | A collapsed reference with a hard break is written full. |
| #211 | 3.3 | A shortcut reference whose text needs escaping is written full. |
| #215 | 3.3 | Link text starting with an escaped `^` is written as a footnote call. |
| #216 | 3.1, 3.3 | A hand-appended separator at the end of an item survives normalizing. |
| #217 | 3.3 | Level-3+ headings: `&amp;#` is not stable. A hand-built break in one is rejected since #257 (1.2). |
| #218 | 3.3 | A root `(` without a final line ending differs in its final line ending. |
| #221 | 5.4 | `render_node` fails partway on a footnote call whose definition is outside the subtree. |
| #222 | 3.3 | A node appended and moved before a paragraph in a tight item makes the list loose. |
| #223 | 3.3 | Inline-root text `[b][c]` writes non-idempotent `**[b][c]**`. |
| #225 | 3.4 | Exception 2 of 3.4: the renderer counts calls inside copies. |
| #226 | 7.2 | Lookups in a detached tree are quadratic over its nodes. |
| #227 | 3.3 | A `*` marker before thematic-break content writes `* ***`, read back as a rule; with `rule: '-'`, `- a\n+ b\n- ***\n` is written `- a\n\n* b\n\n* ---\n`, whose last two lists read back as one. |
| #228 | parser, 3.3 | A paragraph after a quote ending in an empty item leaves the outer list, unlike comrak and markdown-rs, and the writer's output for it does not read back. |
| #229 | 3.3 | An HTML block with an indented first line changes container when written: `1.   a\n   <div>\n` is written `1. a\n\n   <div>\n`, which puts the block in the item. Since #249 the parser also gives one after a definition in an item, `1.   [a]: /u\n   <a href="x">\n`. |
| #231 | 1.2, 3.1 | `attach` puts phrasing directly under an emptied root, and a leading root separator survives normalizing. |
| #233 | 7.2 | Edits deep in a phrasing nest walk every ancestor. |
| #257 | 1.2, 3.3 | Valid inline HTML that does not read back as itself: `\|` or a line ending in a table cell. Values are valid under the document's options (1.2), but the writer writes for the default options and 3.3 reads back with them, so a document under other options, such as one with namespaced tags, blank lines in HTML blocks, or headings without setext, may not read back as itself; writing for the document's options is the next step. `text_mut` can write NUL into text, which the HTML renderer writes as U+FFFD. |
| #230 | enhancement | A table ending a nested item gains a separator when its list is written loose. The HTML and the output are stable, and 3.3 compares without separators, so this breaks no clause; the tree does not keep its separators. |
| #180 | enhancement | Round trip under `ParseOptions::inline_code_info`, which the writer does not know (5.2): text ending in `word:` before code reads back as a language. |
| #185 | enhancement | Round trip under `inline_code_info` with `_` markers next to a language (5.2). |
| #201 | enhancement | Bring the test suite toward 1–2 s. |
| #212 | enhancement | `Document::parse` is about 2x slower on label-dense input since labels moved into events; still linear. |
