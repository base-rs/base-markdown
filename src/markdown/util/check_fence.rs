// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-fence.js
use crate::markdown::state::State;
use crate::message::Message;
use alloc::format;

pub fn check_fence(state: &mut State) -> Result<char, Message> {
    let marker = state.options.fence;

    if marker != '`' && marker != '~' {
        return Err(Message {
            span: None,
            reason: format!(
                "Cannot serialize code with `{}` for `options.fence`, expected `` ` `` or `~`",
                marker
            )
            .into(),
            rule_id: "unexpected-marker",
            source: "mdast-util-to-markdown",
        });
    }

    Ok(marker)
}
