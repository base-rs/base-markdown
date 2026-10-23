// Released under the MIT License.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Deal with several changes in events, batching them together.
//!
//! Preferably, changes should be kept to a minimum.
//! Sometimes, it’s needed to change the list of events, because parsing can be
//! messy, and it helps to expose a cleaner interface of events to the compiler
//! and other users.
//! It can also help to merge many adjacent similar events.
//! And, in other cases, it’s needed to parse subcontent: pass some events
//! through another tokenizer and inject the result.
use crate::event::Event;
use alloc::{vec, vec::Vec};

/// Shift `previous` and `next` links according to `jumps`.
///
/// This fixes links in case there are events removed or added between them.
fn shift_links(events: &mut [Event], jumps: &[(usize, usize, usize)]) {
    let mut jump_index = 0;
    let mut index = 0;
    let mut add = 0;
    let mut rm = 0;

    while index < events.len() {
        let rm_curr = rm;

        while jump_index < jumps.len() && jumps[jump_index].0 <= index {
            add = jumps[jump_index].2;
            rm = jumps[jump_index].1;
            jump_index += 1;
        }

        // Ignore items that will be removed.
        if rm > rm_curr {
            index += rm - rm_curr;
        } else {
            if let Some(link) = &events[index].link {
                if let Some(next) = link.next() {
                    events[next]
                        .link
                        .as_mut()
                        .unwrap()
                        .set_previous(Some(index + add - rm));

                    while jump_index < jumps.len() && jumps[jump_index].0 <= next {
                        add = jumps[jump_index].2;
                        rm = jumps[jump_index].1;
                        jump_index += 1;
                    }

                    events[index]
                        .link
                        .as_mut()
                        .unwrap()
                        .set_next(Some(next + add - rm));
                    index = next;
                    continue;
                }
            }

            index += 1;
        }
    }
}

/// Tracks a bunch of edits.
#[derive(Debug)]
pub struct EditMap {
    /// Record of changes.
    map: Vec<(usize, usize, Vec<Event>)>,
    /// Locate an existing edit without scanning all preceding edits.
    indices: Vec<usize>,
}

impl EditMap {
    /// Create a new edit map.
    pub fn new() -> EditMap {
        EditMap {
            map: vec![],
            indices: vec![],
        }
    }
    /// Create an edit: a remove and/or add at a certain place.
    pub fn add(&mut self, index: usize, remove: usize, add: Vec<Event>) {
        add_impl(self, index, remove, add, false);
    }
    /// Create an edit: but insert `add` before existing additions.
    pub fn add_before(&mut self, index: usize, remove: usize, add: Vec<Event>) {
        add_impl(self, index, remove, add, true);
    }
    /// Done, change the events.
    pub fn consume(&mut self, events: &mut Vec<Event>) {
        self.map
            .sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        if self.map.is_empty() {
            return;
        }

        // Calculate jumps: where items in the current list move to.
        let mut jumps = Vec::with_capacity(self.map.len());
        let mut index = 0;
        let mut add_acc = 0;
        let mut remove_acc = 0;
        while index < self.map.len() {
            let (at, remove, add) = &self.map[index];
            remove_acc += remove;
            add_acc += add.len();
            jumps.push((*at, remove_acc, add_acc));
            index += 1;
        }

        shift_links(events, &jumps);

        // Move each surviving event once, rather than allocating and copying a separate vector for every unchanged span.
        let mut result = Vec::with_capacity(events.len() + add_acc - remove_acc);
        let mut source = core::mem::take(events).into_iter();
        let mut cursor = 0;
        for (at, remove, mut additions) in self.map.drain(..) {
            result.extend(source.by_ref().take(at - cursor));
            for _ in 0..remove {
                source.next();
            }
            result.append(&mut additions);
            cursor = at + remove;
        }
        result.extend(source);
        *events = result;

        self.indices.clear();
    }
}

/// Create an edit.
fn add_impl(edit_map: &mut EditMap, at: usize, remove: usize, mut add: Vec<Event>, before: bool) {
    if remove == 0 && add.is_empty() {
        return;
    }

    if let Some(&index) = edit_map
        .indices
        .get(at)
        .filter(|&&index| index != usize::MAX)
    {
        edit_map.map[index].1 += remove;

        if before {
            add.append(&mut edit_map.map[index].2);
            edit_map.map[index].2 = add;
        } else {
            edit_map.map[index].2.append(&mut add);
        }
    } else {
        if at >= edit_map.indices.len() {
            edit_map.indices.resize(at + 1, usize::MAX);
        }
        edit_map.indices[at] = edit_map.map.len();
        edit_map.map.push((at, remove, add));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Kind, Name, Point};

    fn event(index: usize) -> Event {
        Event {
            kind: Kind::Enter,
            name: Name::Data,
            point: Point {
                line: 1,
                column: index + 1,
                index,
                vs: 0,
            },
            link: None,
        }
    }

    #[test]
    fn combines_edits_in_call_order_and_reuses_indices() {
        let mut events = (0..6).map(event).collect();
        let mut map = EditMap::new();
        map.add(4, 1, vec![event(40)]);
        map.add(1, 1, vec![event(10)]);
        map.add(4, 0, vec![event(41)]);
        map.add_before(4, 0, vec![event(39)]);
        map.add(0, 0, vec![]);
        map.consume(&mut events);
        assert_eq!(
            events.iter().map(|e| e.point.index).collect::<Vec<_>>(),
            vec![0, 10, 2, 3, 39, 40, 41, 5]
        );

        // Sorting during consumption must not leave stale indices behind.
        map.add(1, 2, vec![]);
        map.add(1, 1, vec![event(11)]);
        map.consume(&mut events);
        assert_eq!(
            events.iter().map(|e| e.point.index).collect::<Vec<_>>(),
            vec![0, 11, 39, 40, 41, 5]
        );
    }
}
