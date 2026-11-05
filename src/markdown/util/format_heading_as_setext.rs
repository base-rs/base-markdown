// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Which headings must be written as setext headings.
use crate::{
    markdown::{state::State, LineWrapping},
    stream::{EventKind, Tag},
    Document, NodeId,
};

pub fn format_heading_as_setext<S: AsRef<str>>(
    document: &Document<S>,
    id: NodeId,
    state: &State,
) -> bool {
    heading_as_setext(
        document,
        id,
        state.options.line_wrapping == LineWrapping::Unwrap,
        false,
    )
}

/// Whether heading `id` must be written as a setext heading, when `unwrap` is whether soft breaks
/// are written as spaces. Headings are written ATX, but only a setext heading can hold a line
/// ending, so a heading of rank 1 or 2 with text that holds one is written setext. `encode` is
/// whether a line ending in text, including image text, is written in an ATX heading as `&#xA;`,
/// which reads back as the same text, rather than making the heading setext. Only a heading that
/// a setext heading cannot be written for without a blank line does that (#168), so that other
/// headings are written as before.
pub(crate) fn heading_as_setext<S: AsRef<str>>(
    document: &Document<S>,
    id: NodeId,
    unwrap: bool,
    encode: bool,
) -> bool {
    if !matches!(
        document.node(id).unwrap().kind,
        EventKind::Start(Tag::Heading(1 | 2))
    ) {
        return false;
    }
    let mut nonempty = false;
    let mut setext = false;
    // Unwrapping writes soft breaks and line endings in text as spaces, so they no longer need a multi-line heading. Line endings in inline code are always written as spaces, except in image text, which writes them as alt text, and a code block cannot be written in a heading.
    // Walk the descendants in document order, counting the images that enclose each.
    let mut images = 0;
    let mut current = document.node(id).unwrap().first_child();
    while let Some(child) = current {
        let node = document.node(child).unwrap();
        let kind = &node.kind;
        if let Some(value) = crate::headings::semantic_text::<false>(kind, document.source()) {
            nonempty |= !value.is_empty();
            let unwrapped = unwrap && matches!(kind, EventKind::Text(_) | EventKind::SoftBreak(_));
            let literal = matches!(kind, EventKind::CodeBlock { .. })
                || images == 0 && matches!(kind, EventKind::Code { .. });
            setext = setext
                || matches!(kind, EventKind::HardBreak)
                || !unwrapped && matches!(kind, EventKind::SoftBreak(_))
                || !unwrapped
                    && !literal
                    && !(encode && matches!(kind, EventKind::Text(_)))
                    && value.contains('\n');
        }
        if nonempty && setext {
            return true;
        }
        if let Some(first) = node.first_child() {
            images += usize::from(image(kind));
            current = Some(first);
            continue;
        }
        let mut at = child;
        current = loop {
            let node = document.node(at).unwrap();
            if let Some(next) = node.next_sibling() {
                break Some(next);
            }
            let parent = node.parent().expect("a descendant has a parent");
            if parent == id {
                break None;
            }
            images -= usize::from(image(&document.node(parent).unwrap().kind));
            at = parent;
        };
    }
    false
}

fn image(kind: &EventKind<'_>) -> bool {
    matches!(kind, EventKind::Start(Tag::Image { .. }))
}

#[cfg(test)]
mod tests {
    use super::format_heading_as_setext;
    use crate::{
        markdown::state::State,
        stream::{EventKind, Tag, Text},
        Document, LineWrapping, MarkdownOptions, ParseOptions,
    };

    fn setext(rank: u8, children: &[EventKind<'static>], line_wrapping: LineWrapping) -> bool {
        let mut document = Document::parse("", &ParseOptions::default()).unwrap();
        let heading = document
            .append_child(document.root(), EventKind::Start(Tag::Heading(rank)))
            .unwrap();
        for child in children {
            document.append_child(heading, child.clone()).unwrap();
        }
        let options = MarkdownOptions {
            line_wrapping,
            ..MarkdownOptions::default()
        };
        format_heading_as_setext(&document, heading, &State::new(&options))
    }

    fn eligible(rank: u8, children: &[EventKind<'static>]) -> bool {
        setext(rank, children, LineWrapping::Preserve)
    }

    #[test]
    fn setext_requires_a_line_ending_in_nonempty_text_of_a_shallow_heading() {
        for children in [
            alloc::vec![],
            alloc::vec![EventKind::Text(Text::Literal(""))],
            alloc::vec![EventKind::Start(Tag::Emphasis)],
            alloc::vec![EventKind::FootnoteReference(Text::Literal("label"))],
        ] {
            assert!(!eligible(1, &children));
        }
        for rank in 1..=6 {
            for text in ["plain", " "] {
                assert!(!eligible(rank, &[EventKind::Text(Text::Literal(text))]));
            }
            assert_eq!(
                eligible(rank, &[EventKind::Text(Text::Literal("line\nbreak"))]),
                rank <= 2,
            );
        }
    }

    #[test]
    fn inline_breaks_require_setext_but_code_breaks_do_not() {
        for kind in [
            EventKind::Text(Text::Literal("first\nsecond")),
            EventKind::Html {
                value: Text::Literal("first\nsecond"),
                block: false,
            },
            EventKind::SoftBreak(Text::Literal("\n")),
            EventKind::HardBreak,
        ] {
            assert!(eligible(2, &[kind]));
        }
        // Code writes each line ending as a space.
        assert!(!eligible(
            2,
            &[EventKind::Code {
                value: Text::Literal("first\r\nsecond"),
                language: None,
            }]
        ));

        // Synthetic soft breaks still request setext, but their text may be empty.
        let empty_break = EventKind::SoftBreak(Text::Literal(""));
        assert!(!eligible(2, core::slice::from_ref(&empty_break)));
        let text = EventKind::Text(Text::Literal("text"));
        assert!(eligible(2, &[empty_break.clone(), text.clone()]));
        assert!(eligible(2, &[text, empty_break]));
    }

    #[test]
    fn unwrapped_soft_breaks_do_not_require_setext() {
        let unwrapped = |children: &[EventKind<'static>]| setext(2, children, LineWrapping::Unwrap);
        let text = EventKind::Text(Text::Literal("text"));
        assert!(!unwrapped(&[
            text.clone(),
            EventKind::SoftBreak(Text::Literal("\n")),
            EventKind::Text(Text::Literal("first\nsecond")),
        ]));
        assert!(unwrapped(&[
            text.clone(),
            EventKind::HardBreak,
            text.clone()
        ]));
        assert!(unwrapped(&[EventKind::Html {
            value: Text::Literal("first\nsecond"),
            block: false,
        }]));
    }
}
