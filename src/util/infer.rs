// Released under the MIT License.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Infer things from events.
//!
//! Shared by semantic event producers.
use crate::event::{Event, Kind, Name};
use crate::AlignKind;
use alloc::{vec, vec::Vec};

/// Figure out if a list is spread or not.
///
/// Includes blank lines within items when deciding whether the whole list is loose.
pub fn list_loose(events: &[Event], mut index: usize) -> bool {
    let mut balance = 0;
    let name = &events[index].name;
    debug_assert!(
        match name {
            Name::ListOrdered | Name::ListUnordered => true,
            _ => unreachable!("list inference starts at a list event"),
        },
        "expected list"
    );

    while index < events.len() {
        let event = &events[index];

        if event.kind == Kind::Enter {
            balance += 1;

            if balance == 2 && event.name == Name::ListItem && list_item_loose(events, index) {
                return true;
            }
        } else {
            balance -= 1;

            if balance == 1 && event.name == Name::BlankLineEnding {
                // Blank line directly after item, which is just a prefix.
                //
                // ```markdown
                // > | -␊
                //      ^
                //   | - a
                // ```
                let mut at_empty_list_item = false;
                // Blank line at block quote prefix:
                //
                // ```markdown
                // > | * >␊
                //        ^
                //   | * a
                // ```
                let mut at_empty_block_quote = false;

                // List.
                let mut before = index - 2;

                if events[before].name == Name::ListItem {
                    before -= 1;

                    if events[before].name == Name::SpaceOrTab {
                        before -= 2;
                    }

                    if events[before].name == Name::BlockQuote
                        && events[before - 1].name == Name::BlockQuotePrefix
                    {
                        at_empty_block_quote = true;
                    } else if events[before].name == Name::ListItemPrefix {
                        at_empty_list_item = true;
                    }
                }

                if !at_empty_list_item && !at_empty_block_quote {
                    return true;
                }
            }

            // Done.
            if balance == 0 && event.name == *name {
                break;
            }
        }

        index += 1;
    }

    false
}

/// Figure out if an item is spread or not.
pub fn list_item_loose(events: &[Event], mut index: usize) -> bool {
    debug_assert!(
        match events[index].name {
            Name::ListItem => true,
            _ => unreachable!("list item inference starts at a list item event"),
        },
        "expected list item"
    );
    let mut balance = 0;

    while index < events.len() {
        let event = &events[index];

        if event.kind == Kind::Enter {
            balance += 1;
        } else {
            balance -= 1;

            if balance == 1 && event.name == Name::BlankLineEnding {
                // Blank line directly after a prefix:
                //
                // ```markdown
                // > | -␊
                //      ^
                //   |   a
                // ```
                let mut at_prefix = false;

                // List item.
                let mut before = index - 2;

                if events[before].name == Name::SpaceOrTab {
                    before -= 2;
                }

                if events[before].name == Name::ListItemPrefix {
                    at_prefix = true;
                }

                if !at_prefix {
                    return true;
                }
            }

            // Done.
            if balance == 0 && event.name == Name::ListItem {
                break;
            }
        }

        index += 1;
    }

    false
}

/// Figure out the alignment of a table.
pub fn table_align(events: &[Event], mut index: usize) -> Vec<AlignKind> {
    debug_assert!(
        match events[index].name {
            Name::Table => true,
            _ => unreachable!("table alignment inference starts at a table event"),
        },
        "expected table"
    );
    let mut in_delimiter_row = false;
    let mut align = vec![];

    while index < events.len() {
        let event = &events[index];

        if in_delimiter_row {
            if event.kind == Kind::Enter {
                // Start of alignment value: set a new column.
                if event.name == Name::TableDelimiterCellValue {
                    align.push(if events[index + 1].name == Name::TableDelimiterMarker {
                        AlignKind::Left
                    } else {
                        AlignKind::None
                    });
                }
            } else {
                // End of alignment value: change the column.
                if event.name == Name::TableDelimiterCellValue {
                    if events[index - 1].name == Name::TableDelimiterMarker {
                        let align_index = align.len() - 1;
                        align[align_index] = if align[align_index] == AlignKind::Left {
                            AlignKind::Center
                        } else {
                            AlignKind::Right
                        }
                    }
                }
                // Done!
                else if event.name == Name::TableDelimiterRow {
                    break;
                }
            }
        } else if event.kind == Kind::Enter && event.name == Name::TableDelimiterRow {
            in_delimiter_row = true;
        }

        index += 1;
    }

    align
}
