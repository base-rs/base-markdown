// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Reusable source-backed inline workspace for the active block.
use crate::{
    definitions::Definitions,
    inline_parser::InlineParser,
    stream::{Event, StreamError},
    ParseOptions,
};
use core::ops::Range;

pub(crate) struct InlineWorkspace<'a> {
    source: &'a str,
    options: &'a ParseOptions,
    definitions: Definitions,
    parser: InlineParser,
}

impl<'a> InlineWorkspace<'a> {
    pub fn new(source: &'a str, options: &'a ParseOptions, definitions: Definitions) -> Self {
        Self {
            source,
            options,
            definitions,
            parser: InlineParser::default(),
        }
    }
    pub fn source(&self) -> &'a str {
        self.source
    }
    pub fn options(&self) -> &'a ParseOptions {
        self.options
    }
    pub fn definitions(&self) -> &Definitions {
        &self.definitions
    }

    pub fn region<const EXTENSIONS: bool>(
        &mut self,
        fragments: impl Iterator<Item = Range<usize>> + Clone,
        task: bool,
        table: bool,
    ) {
        if self.definitions.is_empty() {
            self.parser.prepare::<false, EXTENSIONS>(
                self.source,
                fragments,
                &self.definitions,
                self.options,
                task,
                table,
            );
        } else {
            self.parser.prepare::<true, EXTENSIONS>(
                self.source,
                fragments,
                &self.definitions,
                self.options,
                task,
                table,
            );
        }
    }

    #[inline]
    pub fn next_region(&mut self) -> Option<Result<Event<'static>, StreamError>> {
        self.parser
            .next(self.source, &self.definitions, self.options)
            .map(Ok)
    }
}
