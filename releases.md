# Releases

## Unreleased

- Unify all frontmatter under `Node::Frontmatter`, replacing `Node::Yaml`, `Node::Toml`, and their structs. Update Rust matches and struct literals; serialized ASTs now use `type: "frontmatter"` with info and fence fields.
- Return YAML/TOML defaults from `Frontmatter::language()` and `Node::code_language()` for untagged dash/plus delimiters, while keeping the authored info string empty.
- Retain the trailing body line ending for untagged YAML/TOML, matching tagged frontmatter, and serialize all forms through the same handler. Edited bodies that require a different fence retain their effective language.
- Clarify frontmatter placement and serialization, and provide runnable AST renderer examples.

## v0.6.0

- Rename `HTMLRenderer` to `HtmlRenderer` to follow Rust acronym casing. Update imports and type references.

## v0.5.0

- Add `Node::code_fence()` and `mdast::CodeFence`, exposing an opening fence's character, length, and indentation in columns, following Markly's fence structure.
- Retain fence metadata on ordinary code blocks independently of source positions, and expose existing frontmatter fences through the same helper. Inline and indented code return `None`.
- Add `Code::fence: Option<CodeFence>`; Rust struct literals must supply this field (use `None` when no source metadata is available). Older serialized ASTs remain readable.

## v0.4.0

- Add language-agnostic frontmatter through the existing `Constructs::frontmatter` option: closed, language-tagged backtick/tilde fences at document start and format hints on `---`/`+++`.

- Add `Node::Frontmatter` with a raw body, opaque info string, and opening/closing fence metadata; preserve legacy untagged YAML/TOML nodes. Exhaustive matches on `Node` must handle the new variant.

- Preserve generic frontmatter during Markdown serialization, protect edited bodies with safe fences, and omit it from HTML.

- Document the fork's motivation, optional parsing extensions, and AST and rendering additions.

## v0.3.2

- Name renderer source files after their public types without changing public import paths.

## v0.3.1

- Adopt `socketry-project` 0.3.7 for shared project tasks and Markdown normalization.
- Require the aggregate test and coverage result for pull request merges.
- Refresh dependency examples and repository-owned agent guidance.

## v0.3.0

- Use hyphens as the default marker for unordered lists in Markdown serialization.

## v0.2.0

- Add an option to unwrap soft line breaks when serializing Markdown.

- Preserve checked and unchecked task list items when serializing Markdown.

- Fix a panic when serializing long inline code or math values containing CRLF before unsafe Markdown characters.

- Fix parsing and serialization edge cases for HTML blocks, inline code, and Unicode escapes.

## v0.1.1

- Update agent guidance to install dependency context and skills without generating `agents.md`.

## v0.1.0

- Add AST renderers for HTML and Markdown, including Markdown serialization on nodes and fragments.
- Add fragment extraction, heading helpers, and parser fixes from the Socketry fork.
- Adopt shared Socketry testing, documentation, and release automation.
