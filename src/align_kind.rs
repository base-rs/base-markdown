// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

#[cfg(feature = "serde")]
use core::fmt;

/// GFM: alignment of phrasing content.
///
/// Used to align the contents of table cells within a table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlignKind {
    /// Left alignment.
    ///
    /// See the `left` value of the `text-align` CSS property.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | :-- |
    ///       ^^^
    /// ```
    Left,
    /// Right alignment.
    ///
    /// See the `right` value of the `text-align` CSS property.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | --: |
    ///       ^^^
    /// ```
    Right,
    /// Center alignment.
    ///
    /// See the `center` value of the `text-align` CSS property.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | :-: |
    ///       ^^^
    /// ```
    Center,
    /// No alignment.
    ///
    /// Phrasing content is aligned as defined by the host environment.
    ///
    /// ```markdown
    ///   | | aaa |
    /// > | | --- |
    ///       ^^^
    /// ```
    None,
}

/// Implement serde according to <https://github.com/syntax-tree/mdast#aligntype>
#[cfg(feature = "serde")]
impl serde::ser::Serialize for AlignKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        match self {
            AlignKind::Left => serializer.serialize_unit_variant("AlignKind", 0, "left"),
            AlignKind::Right => serializer.serialize_unit_variant("AlignKind", 1, "right"),
            AlignKind::Center => serializer.serialize_unit_variant("AlignKind", 2, "center"),
            AlignKind::None => serializer.serialize_none(),
        }
    }
}

#[cfg(feature = "serde")]
struct AlignKindVisitor;

#[cfg(feature = "serde")]
impl serde::de::Visitor<'_> for AlignKindVisitor {
    type Value = AlignKind;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("'left', 'right', 'center' or null")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v {
            "left" => Ok(AlignKind::Left),
            "right" => Ok(AlignKind::Right),
            "center" => Ok(AlignKind::Center),
            &_ => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Str(v),
                &self,
            )),
        }
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v {
            b"left" => Ok(AlignKind::Left),
            b"right" => Ok(AlignKind::Right),
            b"center" => Ok(AlignKind::Center),
            &_ => Err(serde::de::Error::invalid_type(
                serde::de::Unexpected::Bytes(v),
                &self,
            )),
        }
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(AlignKind::None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(AlignKind::None)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::de::Deserialize<'de> for AlignKind {
    fn deserialize<D>(deserializer: D) -> Result<AlignKind, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer.deserialize_any(AlignKindVisitor)
    }
}
