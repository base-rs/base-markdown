// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-rule.js
use crate::markdown::state::State;
use crate::message::Message;
use alloc::format;

pub fn check_rule(state: &State) -> Result<char, Message> {
    let marker = state.options.rule;

    if marker != '*' && marker != '-' && marker != '_' {
        return Err(Message {
            span: None,
            reason: format!(
                "Cannot serialize rules with `{}` for `options.rule`, expected `*`, `-`, or `_`",
                marker
            )
            .into(),
            rule_id: "unexpected-marker",
            source: "mdast-util-to-markdown",
        });
    }

    Ok(marker)
}
