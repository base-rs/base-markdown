// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Reusable compact inline storage, promoted once when a region needs grammar tokenization.
use crate::{
    definitions::Definitions,
    grammar_parser::GrammarParser,
    inline_parser::InlineParser,
    stream::{Event, StreamError},
    ParseOptions,
};
use alloc::boxed::Box;
use core::ops::Range;

// Keep compact storage inline; grammar promotion allocates its workspace once.
#[allow(clippy::large_enum_variant)]
pub(crate) enum InlineWorkspace<'a> {
    Compact {
        source: &'a str,
        options: &'a ParseOptions,
        definitions: Definitions,
        parser: InlineParser,
    },
    Grammar(Box<GrammarParser<'a>>),
}

impl<'a> InlineWorkspace<'a> {
    pub fn new(source: &'a str, options: &'a ParseOptions, definitions: Definitions) -> Self {
        Self::Compact {
            source,
            options,
            definitions,
            parser: InlineParser::default(),
        }
    }

    pub fn source(&self) -> &'a str {
        match self {
            Self::Compact { source, .. } => source,
            Self::Grammar(parser) => parser.source(),
        }
    }
    pub fn options(&self) -> &'a ParseOptions {
        match self {
            Self::Compact { options, .. } => options,
            Self::Grammar(parser) => parser.options(),
        }
    }
    pub fn definitions(&self) -> &Definitions {
        match self {
            Self::Compact { definitions, .. } => definitions,
            Self::Grammar(parser) => parser.definitions(),
        }
    }

    pub fn region<const EXTENSIONS: bool>(
        &mut self,
        fragments: impl Iterator<Item = Range<usize>> + Clone,
        task: bool,
    ) {
        let (source, options, definitions, parser) = match self {
            Self::Compact {
                source,
                options,
                definitions,
                parser,
            } => (source, options, definitions, parser),
            Self::Grammar(parser) => {
                parser.region::<EXTENSIONS>(fragments, task);
                return;
            }
        };
        if !options.inline_code_info
            && if definitions.is_empty() {
                parser.prepare::<false, EXTENSIONS>(
                    source,
                    fragments.clone(),
                    definitions,
                    options,
                    task,
                )
            } else {
                parser.prepare::<true, EXTENSIONS>(
                    source,
                    fragments.clone(),
                    definitions,
                    options,
                    task,
                )
            }
        {
            return;
        }
        // Transfer buffers and definitions rather than duplicating them. Later regions
        // still try compact scanning inside this reusable compatibility workspace.
        let mut grammar = GrammarParser::empty(
            source,
            options,
            core::mem::take(definitions),
            core::mem::take(parser),
        );
        grammar.grammar_region(fragments, task);
        *self = Self::Grammar(Box::new(grammar));
    }

    #[inline]
    pub fn next_region(&mut self) -> Option<Result<Event<'static>, StreamError>> {
        match self {
            Self::Compact {
                source,
                definitions,
                parser,
                ..
            } => parser.next(source, definitions).map(Ok),
            Self::Grammar(parser) => parser.next_region(),
        }
    }
}
