// Released under the MIT License.
// Copyright, 2024, by Bnchi.
// Copyright, 2024, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

//! Contexts consulted by escaping or nested-list marker selection.
//!
//! JS equivalent: https://github.com/syntax-tree/mdast-util-to-markdown/blob/fd6a508/index.d.ts#L18.
#[derive(Clone, Copy, PartialEq)]
pub enum ConstructName {
    /// Whole autolink.
    ///
    /// ```markdown
    /// > | <https://example.com> and <admin@example.com>
    ///     ^^^^^^^^^^^^^^^^^^^^^     ^^^^^^^^^^^^^^^^^^^
    /// ```
    Autolink,
    /// Whole block quote.
    ///
    /// ```markdown
    /// > | > a
    ///     ^^^
    /// > | b
    ///     ^
    /// ```
    Blockquote,
    /// Code (fenced) language, when fenced with grave accents.
    ///
    /// ````markdown
    /// > | ```js
    ///        ^^
    ///   | console.log(1)
    ///   | ```
    /// ````
    CodeFencedLangGraveAccent,
    /// Code (fenced) language, when fenced with tildes.
    ///
    /// ````markdown
    /// > | ~~~js
    ///        ^^
    ///   | console.log(1)
    ///   | ~~~
    /// ````
    CodeFencedLangTilde,
    /// Code (fenced) meta string, when fenced with grave accents.
    ///
    /// ````markdown
    /// > | ```js eval
    ///           ^^^^
    ///   | console.log(1)
    ///   | ```
    /// ````
    CodeFencedMetaGraveAccent,
    /// Code (fenced) meta string, when fenced with tildes.
    ///
    /// ````markdown
    /// > | ~~~js eval
    ///           ^^^^
    ///   | console.log(1)
    ///   | ~~~
    /// ````
    CodeFencedMetaTilde,
    /// Destination (literal) (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a]: <b> "c"
    ///          ^^^
    /// > | a ![b](<c> "d") e
    ///            ^^^
    /// ```
    DestinationLiteral,
    /// Destination (raw) (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a]: b "c"
    ///          ^
    /// > | a ![b](c "d") e
    ///            ^
    /// ```
    DestinationRaw,
    /// Whole heading (atx).
    ///
    /// ```markdown
    /// > | # alpha
    ///     ^^^^^^^
    /// ```
    HeadingAtx,
    /// Label (occurs in definitions, image reference, image, link reference,
    /// link).
    ///
    /// ```markdown
    /// > | [a]: b "c"
    ///     ^^^
    /// > | a [b] c
    ///       ^^^
    /// > | a ![b][c] d
    ///       ^^^^
    /// > | a [b](c) d
    ///       ^^^
    /// ```
    Label,
    /// List.
    ///
    /// ```markdown
    /// > | * a
    ///     ^^^
    /// > | 1. b
    ///     ^^^^
    /// ```
    List,
    /// List item.
    ///
    /// ```markdown
    /// > | * a
    ///     ^^^
    /// > | 1. b
    ///     ^^^^
    /// ```
    ListItem,
    /// Math (flow) meta flag.
    ///
    /// ```markdown
    /// > | $$a
    ///       ^
    ///   | b
    ///   | $$
    /// ```
    MathFlowMeta,
    /// Phrasing (occurs in headings, paragraphs, etc).
    ///
    /// ```markdown
    /// > | a
    ///     ^
    /// ```
    Phrasing,
    /// Reference (occurs in image, link).
    ///
    /// ```markdown
    /// > | [a][]
    ///        ^^
    /// ```
    Reference,
    /// Title using single quotes (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a](b 'c')
    ///           ^^^
    /// ```
    TitleApostrophe,
    /// Title using double quotes (occurs in definition, image, link).
    ///
    /// ```markdown
    /// > | [a](b "c")
    ///           ^^^
    /// ```
    TitleQuote,
}
