// Released under the MIT License.
// Copyright, 2024-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Diagnostics use the same byte offsets as documents and events.

use alloc::borrow::Cow;
use core::{fmt, ops::Range};

/// A serializer or custom event producer diagnostic.
///
/// Static messages and identifiers require no allocation. A dynamically formatted
/// reason can be supplied with `String::into`. Spans are half-open byte ranges in
/// the source; consumers can derive line and column numbers when presenting them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Source bytes associated with this diagnostic, when available.
    pub span: Option<Range<usize>>,
    /// Human-readable explanation.
    pub reason: Cow<'static, str>,
    /// Category of message.
    pub rule_id: &'static str,
    /// Namespace of message.
    pub source: &'static str,
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(span) = &self.span {
            write!(f, "{}..{}: ", span.start, span.end)?;
        }
        write!(f, "{} ({}:{})", self.reason, self.source, self.rule_id)
    }
}

#[cfg(test)]
mod tests {
    use super::Message;
    use alloc::{borrow::Cow, format, string::ToString};
    use core::fmt::Write as _;

    struct FailingWriter;

    impl core::fmt::Write for FailingWriter {
        fn write_str(&mut self, _value: &str) -> core::fmt::Result {
            Err(core::fmt::Error)
        }
    }

    #[test]
    fn diagnostics_borrow_static_reasons_and_display_byte_ranges() {
        let mut message = Message {
            span: None,
            reason: "invalid value".into(),
            rule_id: "example",
            source: "parser",
        };
        assert_eq!(message.reason, Cow::Borrowed("invalid value"));
        assert_eq!(message.to_string(), "invalid value (parser:example)");
        assert!(write!(&mut FailingWriter, "{message}").is_err());

        message.span = Some(2..5);
        message.reason = format!("invalid value {}", 42).into();
        assert_eq!(
            message.to_string(),
            "2..5: invalid value 42 (parser:example)"
        );
        assert!(write!(&mut FailingWriter, "{message}").is_err());
    }
}
