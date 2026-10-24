// Released under the MIT License.
// Copyright, 2022-2024, by Titus Wormer.
// Copyright, 2024, by Niklas Begley.
// Copyright, 2025, by Bnchi.
// Copyright, 2026, by Samuel Williams.

//! States of the state machine.
use crate::construct;
use crate::tokenizer::Tokenizer;

/// Result of a state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// Move to [`Name`][] next.
    Next(Name),
    /// Retry in [`Name`][].
    Retry(Name),
    /// The state is successful.
    Ok,
    /// The state is not successful.
    Nok,
}

impl State {
    /// A completed Markdown tokenizer always succeeds; failed recognition is handled by speculative attempts.
    pub fn assert_ok(self) {
        match self {
            State::Ok => (),
            _ => unreachable!("a completed Markdown tokenizer must succeed"),
        }
    }
}

/// Names of states to move to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Name {
    AttentionStart,
    AttentionInside,

    AutolinkStart,
    AutolinkScan,

    BlankLineStart,
    BlankLineAfter,

    BlockQuoteStart,
    BlockQuoteContStart,
    BlockQuoteContBefore,
    BlockQuoteContAfter,

    BomStart,
    BomInside,

    CharacterEscapeStart,
    CharacterEscapeInside,

    CharacterReferenceStart,
    CharacterReferenceOpen,
    CharacterReferenceNumeric,
    CharacterReferenceValue,

    CodeIndentedStart,
    CodeIndentedAtBreak,
    CodeIndentedAfter,
    CodeIndentedFurtherStart,
    CodeIndentedInside,
    CodeIndentedFurtherBegin,
    CodeIndentedFurtherAfter,

    ContentChunkStart,
    ContentChunkInside,
    ContentDefinitionBefore,
    ContentDefinitionAfter,

    DataStart,
    DataInside,
    DataAtBreak,

    DefinitionStart,
    DefinitionBefore,
    DefinitionLabelAfter,
    DefinitionLabelNok,
    DefinitionMarkerAfter,
    DefinitionDestinationBefore,
    DefinitionDestinationAfter,
    DefinitionDestinationMissing,
    DefinitionTitleBefore,
    DefinitionAfter,
    DefinitionAfterWhitespace,
    DefinitionTitleBeforeMarker,
    DefinitionTitleAfter,
    DefinitionTitleAfterOptionalWhitespace,

    DestinationStart,
    DestinationEnclosedBefore,
    DestinationEnclosed,
    DestinationEnclosedEscape,
    DestinationRaw,
    DestinationRawEscape,

    DocumentStart,
    DocumentBeforeFrontmatter,
    DocumentContainerExistingBefore,
    DocumentContainerExistingAfter,
    DocumentContainerNewBefore,
    DocumentContainerNewBeforeNotBlockQuote,
    DocumentContainerNewBeforeNotList,
    DocumentContainerNewBeforeNotFootnoteDefinition,
    DocumentContainerNewAfter,
    DocumentContainersAfter,
    DocumentFlowInside,
    DocumentFlowEnd,

    FlowStart,
    FlowBeforeTable,
    FlowBeforeCodeIndented,
    FlowBeforeRaw,
    FlowBeforeHtml,

    FlowBeforeHeadingAtx,
    FlowBeforeHeadingSetext,
    FlowBeforeThematicBreak,
    FlowAfter,
    FlowBlankLineBefore,
    FlowBlankLineAfter,
    FlowBeforeContent,

    FrontmatterStart,
    FrontmatterOpenSequence,
    FrontmatterOpenAfter,
    FrontmatterInfoBefore,
    FrontmatterInfo,
    FrontmatterAfter,
    FrontmatterContentStart,
    FrontmatterContentInside,
    FrontmatterContentEnd,
    FrontmatterCloseStart,
    FrontmatterCloseBeforeSequence,
    FrontmatterCloseSequence,
    FrontmatterCloseAfter,

    AutolinkLiteralProtocolStart,
    AutolinkLiteralProtocolAfter,
    AutolinkLiteralProtocolPrefixInside,
    AutolinkLiteralProtocolSlashesInside,

    AutolinkLiteralWwwStart,
    AutolinkLiteralWwwAfter,
    AutolinkLiteralWwwPrefixInside,
    AutolinkLiteralWwwPrefixAfter,

    AutolinkLiteralDomainInside,
    AutolinkLiteralDomainAtPunctuation,
    AutolinkLiteralDomainAfter,

    AutolinkLiteralPathInside,
    AutolinkLiteralPathAtPunctuation,
    AutolinkLiteralPathAfter,

    AutolinkLiteralTrail,
    AutolinkLiteralTrailCharRefInside,
    AutolinkLiteralTrailCharRefStart,
    AutolinkLiteralTrailBracketAfter,

    FootnoteDefinitionStart,
    FootnoteDefinitionLabelBefore,
    FootnoteDefinitionLabelAtMarker,
    FootnoteDefinitionLabelInside,
    FootnoteDefinitionLabelEscape,
    FootnoteDefinitionLabelAfter,
    FootnoteDefinitionWhitespaceAfter,
    FootnoteDefinitionContStart,
    FootnoteDefinitionContBlank,
    FootnoteDefinitionContFilled,

    LabelStartFootnoteStart,
    LabelStartFootnoteOpen,

    TaskListItemCheckStart,
    TaskListItemCheckInside,
    TaskListItemCheckClose,
    TaskListItemCheckAfter,
    TaskListItemCheckAfterSpaceOrTab,

    TableStart,
    TableHeadRowBefore,
    TableHeadRowStart,
    TableHeadRowBreak,
    TableHeadRowData,
    TableHeadRowEscape,
    TableHeadDelimiterStart,
    TableHeadDelimiterBefore,
    TableHeadDelimiterCellBefore,
    TableHeadDelimiterValueBefore,
    TableHeadDelimiterLeftAlignmentAfter,
    TableHeadDelimiterFiller,
    TableHeadDelimiterRightAlignmentAfter,
    TableHeadDelimiterCellAfter,
    TableHeadDelimiterNok,

    TableBodyRowStart,
    TableBodyRowBreak,
    TableBodyRowData,
    TableBodyRowEscape,

    HardBreakEscapeStart,
    HardBreakEscapeAfter,

    HeadingAtxStart,
    HeadingAtxBefore,
    HeadingAtxSequenceOpen,
    HeadingAtxAtBreak,
    HeadingAtxSequenceFurther,
    HeadingAtxData,

    HeadingSetextStart,
    HeadingSetextBefore,
    HeadingSetextInside,
    HeadingSetextAfter,

    HtmlFlowStart,
    HtmlFlowBefore,
    HtmlFlowOpen,
    HtmlFlowDeclarationOpen,
    HtmlFlowCommentOpenInside,
    HtmlFlowCdataOpenInside,
    HtmlFlowTagCloseStart,
    HtmlFlowTagName,
    HtmlFlowBasicSelfClosing,
    HtmlFlowCompleteClosingTagAfter,
    HtmlFlowCompleteEnd,
    HtmlFlowCompleteAttributeNameBefore,
    HtmlFlowCompleteAttributeName,
    HtmlFlowCompleteAttributeNameAfter,
    HtmlFlowCompleteAttributeValueBefore,
    HtmlFlowCompleteAttributeValueQuoted,
    HtmlFlowCompleteAttributeValueQuotedAfter,
    HtmlFlowCompleteAttributeValueUnquoted,
    HtmlFlowCompleteAfter,
    HtmlFlowBlankLineBefore,
    HtmlFlowBlankLinesCheck,
    HtmlFlowBlankLinesEnd,
    HtmlFlowBlankLinesEndStart,
    HtmlFlowBlankLinesEndData,
    HtmlFlowContinuation,
    HtmlFlowContinuationDeclarationInside,
    HtmlFlowContinuationAfter,
    HtmlFlowContinuationStart,
    HtmlFlowContinuationBefore,
    HtmlFlowContinuationCommentInside,
    HtmlFlowContinuationRawTagOpen,
    HtmlFlowContinuationRawEndTag,
    HtmlFlowContinuationClose,
    HtmlFlowContinuationCdataInside,
    HtmlFlowContinuationStartNonLazy,

    HtmlTextStart,
    HtmlTextScan,
    HtmlTextLineEndingBefore,
    HtmlTextLineEndingAfter,
    HtmlTextLineEndingAfterPrefix,

    LabelStart,
    LabelAtBreak,
    LabelEolAfter,
    LabelEscape,
    LabelInside,
    LabelNok,

    LabelEndStart,
    LabelEndAfter,
    LabelEndResourceStart,
    LabelEndResourceBefore,
    LabelEndResourceOpen,
    LabelEndResourceDestinationAfter,
    LabelEndResourceDestinationMissing,
    LabelEndResourceBetween,
    LabelEndResourceTitleAfter,
    LabelEndResourceEnd,
    LabelEndOk,
    LabelEndNok,
    LabelEndReferenceFull,
    LabelEndReferenceFullAfter,
    LabelEndReferenceFullMissing,
    LabelEndReferenceNotFull,
    LabelEndReferenceCollapsed,
    LabelEndReferenceCollapsedOpen,

    LabelStartImageStart,
    LabelStartImageOpen,
    LabelStartImageAfter,

    LabelStartLinkStart,

    ListItemStart,
    ListItemBefore,
    ListItemBeforeOrdered,
    ListItemBeforeUnordered,
    ListItemValue,
    ListItemMarker,
    ListItemMarkerAfter,
    ListItemAfter,
    ListItemMarkerAfterFilled,
    ListItemWhitespace,
    ListItemPrefixOther,
    ListItemWhitespaceAfter,
    ListItemContStart,
    ListItemContBlank,
    ListItemContFilled,

    NonLazyContinuationStart,
    NonLazyContinuationAfter,

    ParagraphStart,
    ParagraphLineStart,
    ParagraphInside,

    RawFlowStart,
    RawFlowBeforeSequenceOpen,
    RawFlowSequenceOpen,
    RawFlowInfoBefore,
    RawFlowInfo,
    RawFlowMetaBefore,
    RawFlowMeta,
    RawFlowAtNonLazyBreak,
    RawFlowCloseStart,
    RawFlowBeforeSequenceClose,
    RawFlowSequenceClose,
    RawFlowAfterSequenceClose,
    RawFlowContentBefore,
    RawFlowContentStart,
    RawFlowBeforeContentChunk,
    RawFlowContentChunk,
    RawFlowAfter,

    RawTextStart,
    RawTextSequenceOpen,
    RawTextBetween,
    RawTextData,
    RawTextSequenceClose,

    SpaceOrTabStart,
    SpaceOrTabInside,
    SpaceOrTabAfter,

    SpaceOrTabEolStart,
    SpaceOrTabEolAfterFirst,
    SpaceOrTabEolAfterEol,
    SpaceOrTabEolAtEol,
    SpaceOrTabEolAfterMore,

    StringStart,
    StringBefore,
    StringBeforeData,

    TextStart,
    TextBefore,
    TextBeforeHtml,

    TextBeforeHardBreakEscape,
    TextBeforeLabelStartLink,
    TextBeforeData,

    ThematicBreakStart,
    ThematicBreakBefore,
    ThematicBreakSequence,
    ThematicBreakAtBreak,

    TitleStart,
    TitleBegin,
    TitleAfterEol,
    TitleAtBreak,
    TitleEscape,
    TitleInside,
    TitleNok,
}

#[allow(clippy::too_many_lines)]
/// Call the corresponding state for a state name.
pub fn call(tokenizer: &mut Tokenizer, name: Name) -> State {
    let func = match name {
        Name::AttentionStart => construct::attention::start,
        Name::AttentionInside => construct::attention::inside,

        Name::AutolinkStart => construct::autolink::start,
        Name::AutolinkScan => construct::autolink::scan,

        Name::BlankLineStart => construct::blank_line::start,
        Name::BlankLineAfter => construct::blank_line::after,

        Name::BlockQuoteStart => construct::block_quote::start,
        Name::BlockQuoteContStart => construct::block_quote::cont_start,
        Name::BlockQuoteContBefore => construct::block_quote::cont_before,
        Name::BlockQuoteContAfter => construct::block_quote::cont_after,

        Name::BomStart => construct::partial_bom::start,
        Name::BomInside => construct::partial_bom::inside,

        Name::CharacterEscapeStart => construct::character_escape::start,
        Name::CharacterEscapeInside => construct::character_escape::inside,

        Name::CharacterReferenceStart => construct::character_reference::start,
        Name::CharacterReferenceOpen => construct::character_reference::open,
        Name::CharacterReferenceNumeric => construct::character_reference::numeric,
        Name::CharacterReferenceValue => construct::character_reference::value,

        Name::CodeIndentedStart => construct::code_indented::start,
        Name::CodeIndentedAtBreak => construct::code_indented::at_break,
        Name::CodeIndentedAfter => construct::code_indented::after,
        Name::CodeIndentedFurtherStart => construct::code_indented::further_start,
        Name::CodeIndentedInside => construct::code_indented::inside,
        Name::CodeIndentedFurtherBegin => construct::code_indented::further_begin,
        Name::CodeIndentedFurtherAfter => construct::code_indented::further_after,

        Name::ContentChunkStart => construct::content::chunk_start,
        Name::ContentChunkInside => construct::content::chunk_inside,
        Name::ContentDefinitionBefore => construct::content::definition_before,
        Name::ContentDefinitionAfter => construct::content::definition_after,

        Name::DataStart => construct::partial_data::start,
        Name::DataInside => construct::partial_data::inside,
        Name::DataAtBreak => construct::partial_data::at_break,

        Name::DefinitionStart => construct::definition::start,
        Name::DefinitionBefore => construct::definition::before,
        Name::DefinitionLabelAfter => construct::definition::label_after,
        Name::DefinitionLabelNok => construct::definition::label_nok,
        Name::DefinitionMarkerAfter => construct::definition::marker_after,
        Name::DefinitionDestinationBefore => construct::definition::destination_before,
        Name::DefinitionDestinationAfter => construct::definition::destination_after,
        Name::DefinitionDestinationMissing => construct::definition::destination_missing,
        Name::DefinitionTitleBefore => construct::definition::title_before,
        Name::DefinitionAfter => construct::definition::after,
        Name::DefinitionAfterWhitespace => construct::definition::after_whitespace,
        Name::DefinitionTitleBeforeMarker => construct::definition::title_before_marker,
        Name::DefinitionTitleAfter => construct::definition::title_after,
        Name::DefinitionTitleAfterOptionalWhitespace => {
            construct::definition::title_after_optional_whitespace
        }

        Name::DestinationStart => construct::partial_destination::start,
        Name::DestinationEnclosedBefore => construct::partial_destination::enclosed_before,
        Name::DestinationEnclosed => construct::partial_destination::enclosed,
        Name::DestinationEnclosedEscape => construct::partial_destination::enclosed_escape,
        Name::DestinationRaw => construct::partial_destination::raw,
        Name::DestinationRawEscape => construct::partial_destination::raw_escape,

        Name::DocumentStart => construct::document::start,
        Name::DocumentBeforeFrontmatter => construct::document::before_frontmatter,
        Name::DocumentContainerExistingBefore => construct::document::container_existing_before,
        Name::DocumentContainerExistingAfter => construct::document::container_existing_after,
        Name::DocumentContainerNewBefore => construct::document::container_new_before,
        Name::DocumentContainerNewBeforeNotBlockQuote => {
            construct::document::container_new_before_not_block_quote
        }
        Name::DocumentContainerNewBeforeNotList => {
            construct::document::container_new_before_not_list
        }
        Name::DocumentContainerNewBeforeNotFootnoteDefinition => {
            construct::document::container_new_before_not_footnote_definition
        }
        Name::DocumentContainerNewAfter => construct::document::container_new_after,
        Name::DocumentContainersAfter => construct::document::containers_after,
        Name::DocumentFlowEnd => construct::document::flow_end,
        Name::DocumentFlowInside => construct::document::flow_inside,

        Name::FlowStart => construct::flow::start,
        Name::FlowBeforeTable => construct::flow::before_table,
        Name::FlowBeforeCodeIndented => construct::flow::before_code_indented,
        Name::FlowBeforeRaw => construct::flow::before_raw,
        Name::FlowBeforeHtml => construct::flow::before_html,

        Name::FlowBeforeHeadingAtx => construct::flow::before_heading_atx,
        Name::FlowBeforeHeadingSetext => construct::flow::before_heading_setext,
        Name::FlowBeforeThematicBreak => construct::flow::before_thematic_break,
        Name::FlowAfter => construct::flow::after,
        Name::FlowBlankLineBefore => construct::flow::blank_line_before,
        Name::FlowBlankLineAfter => construct::flow::blank_line_after,
        Name::FlowBeforeContent => construct::flow::before_content,

        Name::FrontmatterStart => construct::frontmatter::start,
        Name::FrontmatterOpenSequence => construct::frontmatter::open_sequence,
        Name::FrontmatterOpenAfter => construct::frontmatter::open_after,
        Name::FrontmatterInfoBefore => construct::frontmatter::info_before,
        Name::FrontmatterInfo => construct::frontmatter::info,
        Name::FrontmatterAfter => construct::frontmatter::after,
        Name::FrontmatterContentStart => construct::frontmatter::content_start,
        Name::FrontmatterContentInside => construct::frontmatter::content_inside,
        Name::FrontmatterContentEnd => construct::frontmatter::content_end,
        Name::FrontmatterCloseStart => construct::frontmatter::close_start,
        Name::FrontmatterCloseBeforeSequence => construct::frontmatter::close_before_sequence,
        Name::FrontmatterCloseSequence => construct::frontmatter::close_sequence,
        Name::FrontmatterCloseAfter => construct::frontmatter::close_after,

        Name::AutolinkLiteralProtocolStart => construct::autolink_literal::protocol_start,
        Name::AutolinkLiteralProtocolAfter => construct::autolink_literal::protocol_after,
        Name::AutolinkLiteralProtocolPrefixInside => {
            construct::autolink_literal::protocol_prefix_inside
        }
        Name::AutolinkLiteralProtocolSlashesInside => {
            construct::autolink_literal::protocol_slashes_inside
        }
        Name::AutolinkLiteralWwwAfter => construct::autolink_literal::www_after,
        Name::AutolinkLiteralWwwStart => construct::autolink_literal::www_start,
        Name::AutolinkLiteralWwwPrefixInside => construct::autolink_literal::www_prefix_inside,
        Name::AutolinkLiteralWwwPrefixAfter => construct::autolink_literal::www_prefix_after,
        Name::AutolinkLiteralDomainInside => construct::autolink_literal::domain_inside,
        Name::AutolinkLiteralDomainAtPunctuation => {
            construct::autolink_literal::domain_at_punctuation
        }
        Name::AutolinkLiteralDomainAfter => construct::autolink_literal::domain_after,
        Name::AutolinkLiteralPathInside => construct::autolink_literal::path_inside,
        Name::AutolinkLiteralPathAtPunctuation => construct::autolink_literal::path_at_punctuation,
        Name::AutolinkLiteralPathAfter => construct::autolink_literal::path_after,
        Name::AutolinkLiteralTrail => construct::autolink_literal::trail,
        Name::AutolinkLiteralTrailCharRefStart => construct::autolink_literal::trail_char_ref_start,
        Name::AutolinkLiteralTrailCharRefInside => {
            construct::autolink_literal::trail_char_ref_inside
        }
        Name::AutolinkLiteralTrailBracketAfter => construct::autolink_literal::trail_bracket_after,

        Name::FootnoteDefinitionStart => construct::footnote_definition::start,
        Name::FootnoteDefinitionLabelBefore => construct::footnote_definition::label_before,
        Name::FootnoteDefinitionLabelAtMarker => construct::footnote_definition::label_at_marker,
        Name::FootnoteDefinitionLabelInside => construct::footnote_definition::label_inside,
        Name::FootnoteDefinitionLabelEscape => construct::footnote_definition::label_escape,
        Name::FootnoteDefinitionLabelAfter => construct::footnote_definition::label_after,
        Name::FootnoteDefinitionWhitespaceAfter => construct::footnote_definition::whitespace_after,
        Name::FootnoteDefinitionContStart => construct::footnote_definition::cont_start,
        Name::FootnoteDefinitionContBlank => construct::footnote_definition::cont_blank,
        Name::FootnoteDefinitionContFilled => construct::footnote_definition::cont_filled,

        Name::LabelStartFootnoteStart => construct::label_start_footnote::start,
        Name::LabelStartFootnoteOpen => construct::label_start_footnote::open,

        Name::TableStart => construct::table::start,
        Name::TableHeadRowBefore => construct::table::head_row_before,
        Name::TableHeadRowStart => construct::table::head_row_start,
        Name::TableHeadRowBreak => construct::table::head_row_break,
        Name::TableHeadRowData => construct::table::head_row_data,
        Name::TableHeadRowEscape => construct::table::head_row_escape,
        Name::TableHeadDelimiterStart => construct::table::head_delimiter_start,
        Name::TableHeadDelimiterBefore => construct::table::head_delimiter_before,
        Name::TableHeadDelimiterCellBefore => construct::table::head_delimiter_cell_before,
        Name::TableHeadDelimiterValueBefore => construct::table::head_delimiter_value_before,
        Name::TableHeadDelimiterLeftAlignmentAfter => {
            construct::table::head_delimiter_left_alignment_after
        }
        Name::TableHeadDelimiterFiller => construct::table::head_delimiter_filler,
        Name::TableHeadDelimiterRightAlignmentAfter => {
            construct::table::head_delimiter_right_alignment_after
        }
        Name::TableHeadDelimiterCellAfter => construct::table::head_delimiter_cell_after,
        Name::TableHeadDelimiterNok => construct::table::head_delimiter_nok,
        Name::TableBodyRowStart => construct::table::body_row_start,
        Name::TableBodyRowBreak => construct::table::body_row_break,
        Name::TableBodyRowData => construct::table::body_row_data,
        Name::TableBodyRowEscape => construct::table::body_row_escape,

        Name::TaskListItemCheckStart => construct::task_list_item_check::start,
        Name::TaskListItemCheckInside => construct::task_list_item_check::inside,
        Name::TaskListItemCheckClose => construct::task_list_item_check::close,
        Name::TaskListItemCheckAfter => construct::task_list_item_check::after,
        Name::TaskListItemCheckAfterSpaceOrTab => {
            construct::task_list_item_check::after_space_or_tab
        }

        Name::HardBreakEscapeStart => construct::hard_break_escape::start,
        Name::HardBreakEscapeAfter => construct::hard_break_escape::after,

        Name::HeadingAtxStart => construct::heading_atx::start,
        Name::HeadingAtxBefore => construct::heading_atx::before,
        Name::HeadingAtxSequenceOpen => construct::heading_atx::sequence_open,
        Name::HeadingAtxAtBreak => construct::heading_atx::at_break,
        Name::HeadingAtxSequenceFurther => construct::heading_atx::sequence_further,
        Name::HeadingAtxData => construct::heading_atx::data,

        Name::HeadingSetextStart => construct::heading_setext::start,
        Name::HeadingSetextBefore => construct::heading_setext::before,
        Name::HeadingSetextInside => construct::heading_setext::inside,
        Name::HeadingSetextAfter => construct::heading_setext::after,

        Name::HtmlFlowStart => construct::html_flow::start,
        Name::HtmlFlowBefore => construct::html_flow::before,
        Name::HtmlFlowOpen => construct::html_flow::open,
        Name::HtmlFlowDeclarationOpen => construct::html_flow::declaration_open,
        Name::HtmlFlowCommentOpenInside => construct::html_flow::comment_open_inside,
        Name::HtmlFlowCdataOpenInside => construct::html_flow::cdata_open_inside,
        Name::HtmlFlowTagCloseStart => construct::html_flow::tag_close_start,
        Name::HtmlFlowTagName => construct::html_flow::tag_name,
        Name::HtmlFlowBasicSelfClosing => construct::html_flow::basic_self_closing,
        Name::HtmlFlowCompleteClosingTagAfter => construct::html_flow::complete_closing_tag_after,
        Name::HtmlFlowCompleteEnd => construct::html_flow::complete_end,
        Name::HtmlFlowCompleteAttributeNameBefore => {
            construct::html_flow::complete_attribute_name_before
        }
        Name::HtmlFlowCompleteAttributeName => construct::html_flow::complete_attribute_name,
        Name::HtmlFlowCompleteAttributeNameAfter => {
            construct::html_flow::complete_attribute_name_after
        }
        Name::HtmlFlowCompleteAttributeValueBefore => {
            construct::html_flow::complete_attribute_value_before
        }
        Name::HtmlFlowCompleteAttributeValueQuoted => {
            construct::html_flow::complete_attribute_value_quoted
        }
        Name::HtmlFlowCompleteAttributeValueQuotedAfter => {
            construct::html_flow::complete_attribute_value_quoted_after
        }
        Name::HtmlFlowCompleteAttributeValueUnquoted => {
            construct::html_flow::complete_attribute_value_unquoted
        }
        Name::HtmlFlowCompleteAfter => construct::html_flow::complete_after,
        Name::HtmlFlowBlankLineBefore => construct::html_flow::blank_line_before,
        Name::HtmlFlowBlankLinesCheck => construct::html_flow::blank_lines_check,
        Name::HtmlFlowBlankLinesEnd => construct::html_flow::blank_lines_end,
        Name::HtmlFlowBlankLinesEndStart => construct::html_flow::blank_lines_end_start,
        Name::HtmlFlowBlankLinesEndData => construct::html_flow::blank_lines_end_data,
        Name::HtmlFlowContinuation => construct::html_flow::continuation,
        Name::HtmlFlowContinuationDeclarationInside => {
            construct::html_flow::continuation_declaration_inside
        }
        Name::HtmlFlowContinuationAfter => construct::html_flow::continuation_after,
        Name::HtmlFlowContinuationStart => construct::html_flow::continuation_start,
        Name::HtmlFlowContinuationBefore => construct::html_flow::continuation_before,
        Name::HtmlFlowContinuationCommentInside => {
            construct::html_flow::continuation_comment_inside
        }
        Name::HtmlFlowContinuationRawTagOpen => construct::html_flow::continuation_raw_tag_open,
        Name::HtmlFlowContinuationRawEndTag => construct::html_flow::continuation_raw_end_tag,
        Name::HtmlFlowContinuationClose => construct::html_flow::continuation_close,
        Name::HtmlFlowContinuationCdataInside => construct::html_flow::continuation_cdata_inside,
        Name::HtmlFlowContinuationStartNonLazy => construct::html_flow::continuation_start_non_lazy,

        Name::HtmlTextStart => construct::html_text::start,
        Name::HtmlTextScan => construct::html_text::scan,
        Name::HtmlTextLineEndingBefore => construct::html_text::line_ending_before,
        Name::HtmlTextLineEndingAfter => construct::html_text::line_ending_after,
        Name::HtmlTextLineEndingAfterPrefix => construct::html_text::line_ending_after_prefix,

        Name::LabelStart => construct::partial_label::start,
        Name::LabelAtBreak => construct::partial_label::at_break,
        Name::LabelEolAfter => construct::partial_label::eol_after,
        Name::LabelEscape => construct::partial_label::escape,
        Name::LabelInside => construct::partial_label::inside,
        Name::LabelNok => construct::partial_label::nok,

        Name::LabelEndStart => construct::label_end::start,
        Name::LabelEndAfter => construct::label_end::after,
        Name::LabelEndResourceStart => construct::label_end::resource_start,
        Name::LabelEndResourceBefore => construct::label_end::resource_before,
        Name::LabelEndResourceOpen => construct::label_end::resource_open,
        Name::LabelEndResourceDestinationAfter => construct::label_end::resource_destination_after,
        Name::LabelEndResourceDestinationMissing => {
            construct::label_end::resource_destination_missing
        }
        Name::LabelEndResourceBetween => construct::label_end::resource_between,
        Name::LabelEndResourceTitleAfter => construct::label_end::resource_title_after,
        Name::LabelEndResourceEnd => construct::label_end::resource_end,
        Name::LabelEndOk => construct::label_end::ok,
        Name::LabelEndNok => construct::label_end::nok,
        Name::LabelEndReferenceFull => construct::label_end::reference_full,
        Name::LabelEndReferenceFullAfter => construct::label_end::reference_full_after,
        Name::LabelEndReferenceFullMissing => construct::label_end::reference_full_missing,
        Name::LabelEndReferenceNotFull => construct::label_end::reference_not_full,
        Name::LabelEndReferenceCollapsed => construct::label_end::reference_collapsed,
        Name::LabelEndReferenceCollapsedOpen => construct::label_end::reference_collapsed_open,

        Name::LabelStartImageStart => construct::label_start_image::start,
        Name::LabelStartImageOpen => construct::label_start_image::open,
        Name::LabelStartImageAfter => construct::label_start_image::after,
        Name::LabelStartLinkStart => construct::label_start_link::start,

        Name::ListItemStart => construct::list_item::start,
        Name::ListItemBefore => construct::list_item::before,
        Name::ListItemBeforeOrdered => construct::list_item::before_ordered,
        Name::ListItemBeforeUnordered => construct::list_item::before_unordered,
        Name::ListItemValue => construct::list_item::value,
        Name::ListItemMarker => construct::list_item::marker,
        Name::ListItemMarkerAfter => construct::list_item::marker_after,
        Name::ListItemAfter => construct::list_item::after,
        Name::ListItemMarkerAfterFilled => construct::list_item::marker_after_filled,
        Name::ListItemWhitespace => construct::list_item::whitespace,
        Name::ListItemWhitespaceAfter => construct::list_item::whitespace_after,
        Name::ListItemPrefixOther => construct::list_item::prefix_other,
        Name::ListItemContStart => construct::list_item::cont_start,
        Name::ListItemContBlank => construct::list_item::cont_blank,
        Name::ListItemContFilled => construct::list_item::cont_filled,

        Name::NonLazyContinuationStart => construct::partial_non_lazy_continuation::start,
        Name::NonLazyContinuationAfter => construct::partial_non_lazy_continuation::after,

        Name::ParagraphStart => construct::paragraph::start,
        Name::ParagraphLineStart => construct::paragraph::line_start,
        Name::ParagraphInside => construct::paragraph::inside,

        Name::RawFlowStart => construct::raw_flow::start,
        Name::RawFlowBeforeSequenceOpen => construct::raw_flow::before_sequence_open,
        Name::RawFlowSequenceOpen => construct::raw_flow::sequence_open,
        Name::RawFlowInfoBefore => construct::raw_flow::info_before,
        Name::RawFlowInfo => construct::raw_flow::info,
        Name::RawFlowMetaBefore => construct::raw_flow::meta_before,
        Name::RawFlowMeta => construct::raw_flow::meta,
        Name::RawFlowAtNonLazyBreak => construct::raw_flow::at_non_lazy_break,
        Name::RawFlowCloseStart => construct::raw_flow::close_start,
        Name::RawFlowBeforeSequenceClose => construct::raw_flow::before_sequence_close,
        Name::RawFlowSequenceClose => construct::raw_flow::sequence_close,
        Name::RawFlowAfterSequenceClose => construct::raw_flow::sequence_close_after,
        Name::RawFlowContentBefore => construct::raw_flow::content_before,
        Name::RawFlowContentStart => construct::raw_flow::content_start,
        Name::RawFlowBeforeContentChunk => construct::raw_flow::before_content_chunk,
        Name::RawFlowContentChunk => construct::raw_flow::content_chunk,
        Name::RawFlowAfter => construct::raw_flow::after,

        Name::RawTextStart => construct::raw_text::start,
        Name::RawTextSequenceOpen => construct::raw_text::sequence_open,
        Name::RawTextBetween => construct::raw_text::between,
        Name::RawTextData => construct::raw_text::data,
        Name::RawTextSequenceClose => construct::raw_text::sequence_close,

        Name::SpaceOrTabStart => construct::partial_space_or_tab::start,
        Name::SpaceOrTabInside => construct::partial_space_or_tab::inside,
        Name::SpaceOrTabAfter => construct::partial_space_or_tab::after,

        Name::SpaceOrTabEolStart => construct::partial_space_or_tab_eol::start,
        Name::SpaceOrTabEolAfterFirst => construct::partial_space_or_tab_eol::after_first,
        Name::SpaceOrTabEolAfterEol => construct::partial_space_or_tab_eol::after_eol,
        Name::SpaceOrTabEolAtEol => construct::partial_space_or_tab_eol::at_eol,
        Name::SpaceOrTabEolAfterMore => construct::partial_space_or_tab_eol::after_more,

        Name::StringStart => construct::string::start,
        Name::StringBefore => construct::string::before,
        Name::StringBeforeData => construct::string::before_data,

        Name::TextStart => construct::text::start,
        Name::TextBefore => construct::text::before,
        Name::TextBeforeHtml => construct::text::before_html,

        Name::TextBeforeHardBreakEscape => construct::text::before_hard_break_escape,
        Name::TextBeforeLabelStartLink => construct::text::before_label_start_link,
        Name::TextBeforeData => construct::text::before_data,

        Name::ThematicBreakStart => construct::thematic_break::start,
        Name::ThematicBreakBefore => construct::thematic_break::before,
        Name::ThematicBreakSequence => construct::thematic_break::sequence,
        Name::ThematicBreakAtBreak => construct::thematic_break::at_break,

        Name::TitleStart => construct::partial_title::start,
        Name::TitleBegin => construct::partial_title::begin,
        Name::TitleAfterEol => construct::partial_title::after_eol,
        Name::TitleAtBreak => construct::partial_title::at_break,
        Name::TitleEscape => construct::partial_title::escape,
        Name::TitleInside => construct::partial_title::inside,
        Name::TitleNok => construct::partial_title::nok,
    };

    func(tokenizer)
}
