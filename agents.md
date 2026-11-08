# Agent

## Project Context

- [Markdown Parser Architecture](context/markdown-architecture.md) — parser, AST renderer, serializer, and test-corpus structure.
- [Round-trip Fuzz Harness](context/roundtrip-fuzz.md) — fixed-seed round-trip corpora, the failure baseline, and comparing a branch with main.
- [Normalize and Serialize Contract](context/normalize-serialize.md) — the numbered clauses `normalize`, `to_markdown` and rendering guarantee, the known gaps, and `tests/contract.rs`.

## Agent Context

Run `cargo bake agent:context:install` to install context and skills from Cargo dependencies, and use `cargo bake agent:context:list` and `cargo bake agent:context:show` to discover guidance. Read the skills that apply to the task.

Before planning or changing behavior, discover and read applicable `design/*.md`. Use `base-design-review` to assess plans and changes, and `base-design-author` when user requirements are introduced, changed, or removed. Review design compliance before opening or updating a pull request.

Use `base-security-audit` when reviewing code for security issues, and record each review in `security/audit-*.md`. Use `base-security-advisory` for vulnerability reports and for findings that may warrant an advisory. Before reporting a security finding, read the relevant `// SECURITY:` comments and `design/*.md`.
