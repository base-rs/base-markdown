//! Link against the same library artifact as the release comparison executable.
fn main() {
    println!(
        r#"{{"parser_bytes":{},"node_bytes":{},"builder_bytes":{}}}"#,
        std::mem::size_of::<core_markdown::EventParser<'static>>(),
        std::mem::size_of::<core_markdown::DocumentNode>(),
        std::mem::size_of::<core_markdown::AstBuilder>(),
    );
}
