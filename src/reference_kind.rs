// Released under the MIT License.
// Copyright, 2022-2025, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

/// Explicitness of a reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum ReferenceKind {
    /// The reference is implicit, its identifier inferred from its content.
    Shortcut,
    /// The reference is explicit, its identifier inferred from its content.
    Collapsed,
    /// The reference is explicit, its identifier explicitly set.
    Full,
}
