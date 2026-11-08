# Agent

## Project Context

- [Markdown Parser Architecture](context/markdown-architecture.md) — parser, AST renderer, serializer, and test-corpus structure.
- [Round-trip Fuzz Harness](context/roundtrip-fuzz.md) — fixed-seed round-trip corpora, the failure baseline, and comparing a branch with main.
- [Normalize and Serialize Contract](context/normalize-serialize.md) — the numbered clauses `normalize`, `to_markdown` and rendering guarantee, the known gaps, and `tests/contract.rs`.

## Agent Context

Run `cargo bake agent:context:install` to install context and skills from Cargo dependencies. Read `.agents/context/index.md` and the skills that apply to the task.
