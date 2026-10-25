// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Link against the same library artifact as the release comparison executable.
fn main() {
    println!(
        r#"{{"parser_bytes":{},"node_bytes":{},"builder_bytes":{}}}"#,
        std::mem::size_of::<core_markdown::EventParser<'static>>(),
        std::mem::size_of::<core_markdown::DocumentNode>(),
        std::mem::size_of::<core_markdown::AstBuilder>(),
    );
}
