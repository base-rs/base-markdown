// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Deal with content in other content.
//!
//! To deal with content in content, *you* (a `markdown-rs` contributor) add
//! info on events.
//! Events are a flat list, but they can be connected to each other with a
//! [`Link`][crate::event::Link].
//! Links must occur on [`Enter`][Kind::Enter] events only, which are void
//! (they are followed by their corresponding [`Exit`][Kind::Exit] event).
//!
//! Links will then be passed through a tokenizer for the corresponding content
//! type by `subtokenize`.
//! The subevents they result in are split up into slots for each linked event
//! and replace those links.
//!
//! Subevents are not immediately subtokenized as markdown prevents us from
//! doing so due to definitions, which can occur after references, and thus the
//! whole document needs to be parsed up to the level of definitions, before
//! any level that can include references can be parsed.
use crate::event::{Content, Event, Kind, Name, VOID_EVENTS};
use crate::parser::ParseState;
use crate::state::{Name as StateName, State};
use crate::tokenizer::Tokenizer;
use crate::util::edit_map::EditMap;
use alloc::{string::String, vec, vec::Vec};

#[derive(Debug)]
pub struct Subresult {
    pub footnote_definitions: Vec<String>,
}

/// Link two [`Event`][]s.
///
/// Arbitrary (void) events can be linked together.
/// This optimizes for the common case where the event at `index` is connected
/// to the previous void event.
pub fn link(events: &mut [Event], index: usize) {
    link_to(events, index - 2, index);
}

/// Link two arbitrary [`Event`][]s together.
pub fn link_to(events: &mut [Event], previous: usize, next: usize) {
    debug_assert_eq!(events[previous].kind, Kind::Enter);
    debug_assert!(
        VOID_EVENTS.iter().any(|d| d == &events[previous].name),
        "expected event to be void"
    );
    debug_assert_eq!(events[previous + 1].kind, Kind::Exit);
    debug_assert_eq!(events[previous].name, events[previous + 1].name);
    debug_assert_eq!(events[next].kind, Kind::Enter);
    debug_assert!(
        VOID_EVENTS.iter().any(|d| d == &events[next].name),
        "expected event to be void"
    );
    // Note: the exit of this event may not exist, so don’t check for that.

    let link_previous = events[previous]
        .link
        .as_mut()
        .expect("expected `link` on previous");
    link_previous.set_next(Some(next));
    let link_next = events[next].link.as_mut().expect("expected `link` on next");
    link_next.set_previous(Some(previous));

    debug_assert_eq!(
        events[previous].link.as_ref().unwrap().content,
        events[next].link.as_ref().unwrap().content,
        "expected `content` to match"
    );
}

/// Resolve linked block content or strings; text chunks stay deferred for the semantic producer.
pub fn subtokenize(
    events: &mut Vec<Event>,
    parse_state: &ParseState,
    filter: Option<&Content>,
) -> Subresult {
    let mut map = EditMap::new();
    let mut index = 0;
    let mut value = Subresult {
        footnote_definitions: vec![],
    };
    let mut acc = (0, 0);
    let mut child: Option<Tokenizer> = None;

    while index < events.len() {
        let event = &events[index];

        // Find each first opening chunk.
        if let Some(ref link) = event.link {
            debug_assert_eq!(event.kind, Kind::Enter);

            // No need to enter linked events again.
            if link.previous().is_none()
                && (filter.is_none() || &link.content == *filter.as_ref().unwrap())
            {
                // Definitions start with `[` after optional indentation. Otherwise content is exactly one paragraph, whose line chunks already have the source boundaries needed for text tokenization.
                if link.content == Content::Content
                    && (!parse_state.options.constructs.definition
                        || !matches!(parse_state.bytes[event.point.index], b'[' | b' ' | b'\t'))
                {
                    let start = event.point.clone();
                    let mut current = index;
                    let end = loop {
                        events[current].name = Name::Data;
                        events[current + 1].name = Name::Data;
                        let link = events[current].link.as_mut().unwrap();
                        link.content = Content::Text;
                        if let Some(next) = link.next() {
                            current = next;
                        } else {
                            break events[current + 1].point.clone();
                        }
                    };
                    map.add(
                        index,
                        0,
                        vec![Event {
                            kind: Kind::Enter,
                            name: Name::Paragraph,
                            point: start,
                            link: None,
                        }],
                    );
                    map.add(
                        current + 2,
                        0,
                        vec![Event {
                            kind: Kind::Exit,
                            name: Name::Paragraph,
                            point: end,
                            link: None,
                        }],
                    );
                    // Later subevents are shifted by the two new paragraph boundaries.
                    acc.1 += 2;
                    index += 1;
                    continue;
                }
                // Index into `events` pointing to a chunk.
                let mut link_index = Some(index);
                // Subtokenizer.
                if let Some(tokenizer) = child.as_mut() {
                    tokenizer.reset(event.point.clone());
                } else {
                    child = Some(Tokenizer::new(event.point.clone(), parse_state));
                }
                let tokenizer = child.as_mut().unwrap();
                // Substate.
                let mut state = State::Next(match link.content {
                    Content::Content => StateName::ContentDefinitionBefore,
                    Content::String => StateName::StringStart,
                    Content::Text | Content::Flow => unreachable!(
                        "block preparation resolves content and strings; the semantic producer resolves inline text"
                    ),
                });

                // The semantic producer sets task eligibility before tokenizing an inline region; linked subcontent has no surrounding list context.

                // Loop through links to pass them in order to the subtokenizer.
                while let Some(index) = link_index {
                    let enter = &events[index];
                    let link_curr = enter.link.as_ref().expect("expected link");
                    debug_assert_eq!(enter.kind, Kind::Enter);

                    if link_curr.previous().is_some() {
                        tokenizer.define_skip(enter.point.clone());
                    }

                    let end = &events[index + 1].point;

                    state = tokenizer.push(
                        (enter.point.index, enter.point.vs),
                        (end.index, end.vs),
                        state,
                    );

                    link_index = link_curr.next();
                }

                let mut result = tokenizer.flush(state, true);
                value
                    .footnote_definitions
                    .append(&mut result.footnote_definitions);

                acc = divide_events(&mut map, events, index, &mut tokenizer.events, acc);
            }
        }

        index += 1;
    }

    map.consume(events);

    value
}

/// Divide `child_events` over links in `events`, the first of which is at
/// `link_index`.
pub fn divide_events(
    map: &mut EditMap,
    events: &[Event],
    link_index: usize,
    child_events: &mut Vec<Event>,
    acc_before: (usize, usize),
) -> (usize, usize) {
    let len = child_events.len();
    let slices = linked_slices(events, link_index, child_events, acc_before);

    // Finally, inject the subevents.
    let mut index = slices.len();
    while index > 0 {
        index -= 1;
        map.add(slices[index].0, 2, child_events.split_off(slices[index].1));
    }

    (acc_before.0 + (slices.len() * 2), acc_before.1 + len)
}

/// Assemble a document directly from its container prefixes and flow records. Each buffer is consumed once; line chunks never become separately allocated edit vectors.
pub fn merge_events(events: &mut Vec<Event>, link_index: usize, child_events: &mut Vec<Event>) {
    let slices = linked_slices(events, link_index, child_events, (0, 0));
    let mut result = Vec::with_capacity(events.len() + child_events.len() - slices.len() * 2);
    let mut parent = core::mem::take(events).into_iter();
    let mut child = child_events.drain(..);
    let mut parent_index = 0;
    let mut child_index = 0;
    for (index, &(at, _)) in slices.iter().enumerate() {
        result.extend(parent.by_ref().take(at - parent_index));
        parent.next();
        parent.next();
        parent_index = at + 2;
        let end = slices
            .get(index + 1)
            .map_or(child.len() + child_index, |slice| slice.1);
        result.extend(child.by_ref().take(end - child_index));
        child_index = end;
    }
    result.extend(parent);
    *events = result;
}

/// Locate source-line boundaries and translate inline-chunk links into their final document indices.
fn linked_slices(
    events: &[Event],
    mut link_index: usize,
    child_events: &mut [Event],
    acc_before: (usize, usize),
) -> Vec<(usize, usize)> {
    // Loop through `child_events` to figure out which parts belong where and
    // fix deep links.
    let mut child_index = 0;
    let mut slices = vec![];
    let mut slice_start = 0;
    let mut old_prev: Option<usize> = None;
    let len = child_events.len();

    while child_index < len {
        let current = &child_events[child_index].point;
        let end = &events[link_index + 1].point;

        // Find the first event that starts after the end we’re looking
        // for.
        if current.index > end.index || (current.index == end.index && current.vs > end.vs) {
            slices.push((link_index, slice_start));
            slice_start = child_index;
            link_index = events[link_index].link.as_ref().unwrap().next().unwrap();
        }

        // Fix sublinks.
        if let Some(sublink_curr) = &child_events[child_index].link {
            if sublink_curr.previous().is_some() {
                let old_prev = old_prev.unwrap();
                let prev_event = &mut child_events[old_prev];
                // The `index` in `events` where the current link is,
                // minus one to get the previous link,
                // minus 2 events (the enter and exit) for each removed
                // link.
                let new_link = if slices.is_empty() {
                    old_prev + link_index + 2
                } else {
                    old_prev + link_index - (slices.len() - 1) * 2
                };
                prev_event
                    .link
                    .as_mut()
                    .unwrap()
                    .set_next(Some(new_link + acc_before.1 - acc_before.0));
            }
        }

        // If there is a `next` link in the subevents, we have to change
        // its `previous` index to account for the shifted events.
        // If it points to a next event, we also change the next event’s
        // reference back to *this* event.
        if let Some(sublink_curr) = &child_events[child_index].link {
            if let Some(next) = sublink_curr.next() {
                let sublink_next = child_events[next].link.as_mut().unwrap();

                old_prev = sublink_next.previous();

                sublink_next.set_previous(
                    sublink_next
                        .previous()
                        // The `index` in `events` where the current link is,
                        // minus 2 events (the enter and exit) for each removed
                        // link.
                        .map(|previous| {
                            previous + link_index - (slices.len() * 2) + acc_before.1 - acc_before.0
                        }),
                );
            }
        }

        child_index += 1;
    }

    if !child_events.is_empty() {
        slices.push((link_index, slice_start));
    }

    slices
}

#[cfg(test)]
mod tests {
    #[test]
    fn reused_tokenizers_do_not_leak_construct_state() {
        let paragraphs = [
            "Unclosed [bracket\n",
            "Unclosed *attention\n",
            "Unclosed `code\n",
            "**Strong** and [link](/path \"title\")\n",
            "![image](/image.png) and ~~deleted~~\n",
            "Text with a hard break  \nnext line\n",
            "Text with a tab\tand 世界\n",
        ];
        for options in [crate::Options::default(), crate::Options::gfm()] {
            let expected = paragraphs
                .iter()
                .map(|paragraph| crate::to_html_with_options(paragraph, &options).unwrap())
                .collect::<alloc::string::String>();
            let document = paragraphs.join("\n");
            assert_eq!(
                crate::to_html_with_options(&document, &options).unwrap(),
                expected
            );
        }
    }
}
