# Extension Configuration

Enable Markdown extensions by their syntax names. `Constructs` controls parsing, `ParseOptions` adjusts syntax details, and `CompileOptions` controls HTML output.

```rust
use base_markdown::{to_html_with_options, Constructs, Options, ParseOptions};

let options = Options {
    parse: ParseOptions {
        constructs: Constructs {
            table: true,
            strikethrough: true,
            ..Constructs::default()
        },
        ..ParseOptions::default()
    },
    ..Options::default()
};

let html = to_html_with_options("~~removed~~", &options);
assert_eq!(html, "<p><del>removed</del></p>");
```

The extension constructs are `autolink_literal`, `table`, `strikethrough`, `task_list_item`, `footnote_definition`, and `label_start_footnote`. Enable both footnote constructs to recognize definitions and references. Other optional constructs, including `frontmatter`, `math_flow`, and `math_text`, use the same configuration pattern. Individual extensions retain their existing defaults and parsing behavior.

`ParseOptions::strikethrough_single_tilde` adjusts whether one tilde can delimit strikethrough. HTML options use the same feature names: `CompileOptions::footnote_label`, `footnote_back_label`, `footnote_clobber_prefix`, `footnote_label_attributes`, `footnote_label_tag_name`, `task_list_item_checkable`, and `tagfilter`.

`Constructs::gfm()`, `ParseOptions::gfm()`, `CompileOptions::gfm()`, and `Options::gfm()` remain presets for the GitHub-flavored Markdown combination. Their individual features can still be adjusted afterward. References to GFM in specifications and corpus names identify the dialect being compared.

## Migrating configuration names

Remove `gfm_` from individual extension fields. For example, `constructs.gfm_table` becomes `constructs.table`, `parse.gfm_strikethrough_single_tilde` becomes `parse.strikethrough_single_tilde`, and `compile.gfm_footnote_label` becomes `compile.footnote_label`. Internal modules, tokenizer names, and scratch fields follow the same syntax-based naming; `gfm_footnote_definitions` becomes `footnote_definitions`.

Serialized configuration uses the corresponding names without the `gfm` prefix: `gfmTable` becomes `table`, `gfmStrikethroughSingleTilde` becomes `strikethroughSingleTilde`, and `gfmFootnoteLabel` becomes `footnoteLabel`. Other multiword names continue to use camel case, such as `autolinkLiteral` and `footnoteDefinition`. This is a breaking Rust API and configuration-format rename; update stored configuration and callers together.
