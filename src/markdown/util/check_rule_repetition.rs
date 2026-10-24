// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-rule-repetition.js
use crate::markdown::state::State;
use crate::message::Message;
use alloc::format;

pub fn check_rule_repetition(state: &State) -> Result<u32, Message> {
    let repetition = state.options.rule_repetition;

    if repetition < 3 {
        return Err(Message {
            span: None,
            reason: format!(
                "Cannot serialize rules with repetition `{}` for `options.rule_repetition`, expected `3` or more",
                repetition
            ).into(),
            rule_id: "unexpected-marker",
            source: "mdast-util-to-markdown",
        });
    }

    Ok(repetition)
}
