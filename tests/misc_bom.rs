// Released under the MIT License.
// Copyright, 2022, by Bernhard Berger.
// Copyright, 2022, by Titus Wormer.
// Copyright, 2026, by Samuel Williams.

use core_markdown::to_html;
use pretty_assertions::assert_eq;

#[test]
fn bom() {
    assert_eq!(to_html("\u{FEFF}"), "", "should ignore just a bom");

    assert_eq!(
        to_html("\u{FEFF}# hea\u{FEFF}ding"),
        "<h1>hea\u{FEFF}ding</h1>",
        "should ignore a bom"
    );
}
