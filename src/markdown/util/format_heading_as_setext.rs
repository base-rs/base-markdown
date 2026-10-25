// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Setext heading selection adapted from `mdast-util-to-markdown`.
use crate::{
    markdown::state::State,
    stream::{EventKind, Tag},
    Document, NodeId,
};

pub fn format_heading_as_setext<S: AsRef<str>>(
    document: &Document<S>,
    id: NodeId,
    state: &State,
) -> bool {
    if !matches!(
        document.node(id).unwrap().kind,
        EventKind::Start(Tag::Heading(1 | 2))
    ) {
        return false;
    }
    let mut nonempty = false;
    let mut setext = state.options.setext;
    document.descendants(id).any(|child| {
        let kind = &document.node(child).unwrap().kind;
        if let Some(value) = crate::headings::semantic_text::<false>(kind, document.source()) {
            nonempty |= !value.is_empty();
            setext = setext
                || matches!(kind, EventKind::HardBreak | EventKind::SoftBreak(_))
                || !matches!(kind, EventKind::CodeBlock { .. }) && value.contains(['\r', '\n']);
        }
        nonempty && setext
    })
}

#[cfg(test)]
mod tests {
    use super::format_heading_as_setext;
    use crate::{
        markdown::state::State,
        stream::{EventKind, Tag, Text},
        Document, MarkdownOptions, ParseOptions,
    };

    fn eligible(rank: u8, children: &[EventKind<'static>], setext: bool) -> bool {
        let mut document = Document::parse("", &ParseOptions::default()).unwrap();
        let heading = document
            .append_child(document.root(), EventKind::Start(Tag::Heading(rank)))
            .unwrap();
        for child in children {
            document.append_child(heading, child.clone()).unwrap();
        }
        let options = MarkdownOptions {
            setext,
            ..MarkdownOptions::default()
        };
        format_heading_as_setext(&document, heading, &State::new(&options))
    }

    #[test]
    fn setext_requires_nonempty_semantic_text_in_a_shallow_heading() {
        for children in [
            alloc::vec![],
            alloc::vec![EventKind::Text(Text::Literal(""))],
            alloc::vec![EventKind::Start(Tag::Emphasis)],
            alloc::vec![EventKind::FootnoteReference(Text::Literal("label"))],
        ] {
            for setext in [false, true] {
                assert!(!eligible(1, &children, setext));
            }
        }
        for rank in 1..=6 {
            for text in ["plain", " ", "line\nbreak"] {
                assert_eq!(
                    eligible(rank, &[EventKind::Text(Text::Literal(text))], true),
                    rank <= 2,
                );
            }
        }
        assert!(!eligible(
            1,
            &[EventKind::Text(Text::Literal("plain"))],
            false
        ));
    }

    #[test]
    fn inline_breaks_require_setext_but_code_block_breaks_do_not() {
        for kind in [
            EventKind::Text(Text::Literal("first\nsecond")),
            EventKind::Code {
                value: Text::Literal("first\rsecond"),
                language: None,
            },
            EventKind::Math(Text::Literal("first\r\nsecond")),
            EventKind::Html {
                value: Text::Literal("first\nsecond"),
                block: false,
            },
            EventKind::SoftBreak(Text::Literal("\n")),
            EventKind::HardBreak,
        ] {
            assert!(eligible(2, &[kind], false));
        }
        let block = EventKind::CodeBlock {
            value: Text::Literal("block\n"),
            language: None,
            fence: None,
            math: false,
            unclosed: false,
        };
        assert!(!eligible(2, core::slice::from_ref(&block), false));
        assert!(eligible(2, &[block], true));

        // Synthetic soft breaks still request setext, but their text may be empty.
        let empty_break = EventKind::SoftBreak(Text::Literal(""));
        assert!(!eligible(2, core::slice::from_ref(&empty_break), false));
        let text = EventKind::Text(Text::Literal("text"));
        assert!(eligible(2, &[empty_break.clone(), text.clone()], false));
        assert!(eligible(2, &[text, empty_break], false));
    }
}
