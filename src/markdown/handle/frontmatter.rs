// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::Handle;
use crate::markdown::{
    state::{Info, State},
    util::longest_char_streak::longest_char_streak,
};
use crate::{
    mdast::{Frontmatter, Node},
    message::Message,
};
use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
};

impl Handle for Frontmatter {
    fn handle(
        &self,
        _state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<String, Message> {
        // Validate the fence and the authored format hint.
        let mut info = self.info.trim_matches([' ', '\t']);
        if !matches!(self.fence, '`' | '~' | '-' | '+')
            || (matches!(self.fence, '`' | '~') && info.is_empty())
            || self.info.contains(['\r', '\n'])
            || (self.fence == '`' && self.info.contains('`'))
        {
            return Err(Message {
                place: None,
                reason: "Frontmatter requires a valid fence and a single-line info string; code fences require a language hint".into(),
                rule_id: Box::new("invalid-frontmatter".into()),
                source: Box::new("mdast-util-to-markdown".into()),
            });
        }

        // Choose fences that cannot be closed by the edited body.
        let mut marker = self.fence;
        if matches!(marker, '-' | '+') {
            let delimiter = marker.to_string().repeat(3);
            if self
                .value
                .split(['\r', '\n'])
                .any(|line| line.trim_end_matches([' ', '\t']) == delimiter)
            {
                marker = '~';
            }
        }
        let (opening_length, closing_length) = if matches!(marker, '-' | '+') {
            (3, 3)
        } else {
            let minimum = (longest_char_streak(&self.value, marker) + 1).max(3);
            let opening = self.fence_length.max(minimum);
            (opening, self.closing_fence_length.max(opening))
        };
        let opening = marker.to_string().repeat(opening_length);
        let closing = marker.to_string().repeat(closing_length);

        // Carry the implicit language into a replacement code fence.
        if marker != self.fence && self.info.split_ascii_whitespace().next().is_none() {
            info = self
                .language()
                .expect("dash and plus fences imply a language");
        }

        // Render the authored body between its opening and closing fences.
        let separator =
            if !info.is_empty() && (matches!(marker, '-' | '+') || info.starts_with(marker)) {
                " "
            } else {
                ""
            };
        let mut output = format!("{opening}{separator}{}\n", info);
        output.push_str(&self.value);
        if !self.value.is_empty() && !self.value.ends_with(['\r', '\n']) {
            output.push('\n');
        }
        output.push_str(&closing);
        Ok(output)
    }
}
