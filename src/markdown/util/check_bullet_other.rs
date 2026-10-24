// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! JS equivalent https://github.com/syntax-tree/mdast-util-to-markdown/blob/main/lib/util/check-bullet-other.js
use super::check_bullet::check_bullet;
use crate::markdown::state::State;
use crate::message::Message;
use alloc::format;

pub fn check_bullet_other(state: &mut State) -> Result<char, Message> {
    let bullet = check_bullet(state)?;
    let mut bullet_other = state.options.bullet_other;

    if bullet != '*' {
        bullet_other = '*';
    }

    if bullet_other != '*' && bullet_other != '+' && bullet_other != '-' {
        return Err(Message {
            span: None,
            reason: format!(
                "Cannot serialize items with `{}` for `options.bullet_other`, expected `*`, `+`, or `-`",
                bullet_other
            ).into(),
            rule_id: "unexpected-marker",
            source: "mdast-util-to-markdown",
        });
    }

    if bullet_other == bullet {
        return Err(Message {
            span: None,
            reason: format!(
                "Expected `bullet` (`{}`) and `bullet_other` (`{}`) to be different",
                bullet, bullet_other
            )
            .into(),
            rule_id: "bullet-match-bullet_other",
            source: "mdast-util-to-markdown",
        });
    }

    Ok(bullet_other)
}
