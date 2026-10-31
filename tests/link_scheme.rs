// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

//! Every link scheme in the table, in `<…>`, bare GFM literal, safe `href` and serializer forms.
mod support;
use base_markdown::{
    markdown::to_markdown, to_html, to_html_with_options, CompileOptions, Document, Options,
    ParseOptions,
};
use pretty_assertions::assert_eq;
use support::fixture::canonical;

const HOST_SCHEMES: [&str; 7] = ["http", "https", "ftp", "sftp", "ssh", "irc", "ircs"];
const ADDRESS_SCHEMES: [&str; 2] = ["mailto", "xmpp"];
const PHONE_SCHEMES: [&str; 2] = ["tel", "sms"];

fn danger() -> Options {
    Options {
        compile: CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn link(destination: &str) -> String {
    format!("<p><a href=\"{destination}\">{destination}</a></p>")
}

/// Every accepted angle autolink, with its schemes in lowercase and in mixed case.
fn angle_links() -> Vec<String> {
    let mut links = Vec::new();
    for scheme in HOST_SCHEMES {
        links.push(format!("{scheme}://a.b/c?d#e"));
        links.push(format!("{}://A.B", scheme.to_uppercase()));
    }
    for scheme in ADDRESS_SCHEMES {
        links.push(format!("{scheme}:a.b+c@d-e.f"));
        links.push(format!("{}:A@B", scheme.to_uppercase()));
    }
    for scheme in PHONE_SCHEMES {
        links.push(format!("{scheme}:+1-(555).0100"));
        links.push(format!("{}:5", scheme.to_uppercase()));
    }
    links
}

#[test]
fn angle_autolinks_accept_every_scheme() {
    for destination in angle_links() {
        assert_eq!(
            to_html(&format!("<{destination}>")),
            link(&destination),
            "{destination:?}"
        );
    }
    assert_eq!(
        to_html("<HTTPS://a.b>"),
        link("HTTPS://a.b"),
        "should match schemes case-insensitively"
    );
    assert_eq!(
        to_html("<Tel:+1555>"),
        link("Tel:+1555"),
        "should match phone schemes case-insensitively"
    );
}

#[test]
fn angle_autolinks_reject_what_the_kind_does_not_allow() {
    let mut inputs = vec![
        "<svg:rect>".to_string(),
        "<math:mi>".into(),
        "<xml:lang>".into(),
        "<xmlns:a>".into(),
        "<xlink:href>".into(),
        "<std::io::Error>".into(),
        "<javascript:alert(1)>".into(),
        "<vbscript:a>".into(),
        "<data:text/html,x>".into(),
        "<file:///etc/passwd>".into(),
        "<tel:>".into(),
        "<tel:abc>".into(),
        "<tel:+->".into(),
        "<tel:1 2>".into(),
        "<tel://1>".into(),
        "<ssh:host>".into(),
        "<mailto:a>".into(),
        "<mailto:@a>".into(),
        "<mailto:a@>".into(),
        "<mailto:a@b.>".into(),
        "<mailto:mailto:a@b>".into(),
        "<sshs://a>".into(),
        "<mailtos:a@b>".into(),
    ];
    for scheme in HOST_SCHEMES {
        inputs.push(format!("<{scheme}:a.b>"));
        inputs.push(format!("<{scheme}:/a.b>"));
        inputs.push(format!("<{scheme}://>"));
        inputs.push(format!("<{scheme}:///a>"));
    }
    for scheme in ADDRESS_SCHEMES {
        inputs.push(format!("<{scheme}:>"));
        inputs.push(format!("<{scheme}:a>"));
        inputs.push(format!("<{scheme}:a b@c>"));
    }
    for scheme in PHONE_SCHEMES {
        inputs.push(format!("<{scheme}:>"));
        inputs.push(format!("<{scheme}:a1>"));
        inputs.push(format!("<{scheme}:+>"));
    }
    for input in inputs {
        let expected = format!(
            "<p>&lt;{}&gt;</p>",
            input[1..input.len() - 1].replace('&', "&amp;")
        );
        assert_eq!(to_html(&input), expected, "{input:?}");
        assert_eq!(
            to_html_with_options(&input, &danger()),
            expected,
            "{input:?}"
        );
    }
}

#[test]
fn angle_autolinks_with_letters_only_fall_back_to_email() {
    assert_eq!(
        to_html("<ssh@a.b> <telephone@a.b> <mailto@a.b>"),
        "<p><a href=\"mailto:ssh@a.b\">ssh@a.b</a> <a href=\"mailto:telephone@a.b\">telephone@a.b</a> <a href=\"mailto:mailto@a.b\">mailto@a.b</a></p>",
        "should read a scheme-like local part as an email"
    );
}

#[test]
fn bare_links_accept_host_and_address_schemes() {
    for scheme in HOST_SCHEMES {
        let destination = format!("{scheme}://a.b/c");
        assert_eq!(
            to_html_with_options(&destination, &Options::gfm()),
            link(&destination),
            "{destination:?}"
        );
        let destination = format!("{}://a.b", scheme.to_uppercase());
        assert_eq!(
            to_html_with_options(&format!("x {destination}."), &Options::gfm()),
            format!("<p>x <a href=\"{destination}\">{destination}</a>.</p>"),
            "{destination:?}"
        );
    }
    for scheme in ADDRESS_SCHEMES {
        let destination = format!("{scheme}:a@b.c");
        assert_eq!(
            to_html_with_options(&destination, &Options::gfm()),
            link(&destination),
            "{destination:?}"
        );
    }
    assert_eq!(
        to_html_with_options("SSH://a.b", &Options::gfm()),
        link("SSH://a.b"),
        "should match bare schemes case-insensitively"
    );
    assert_eq!(
        to_html_with_options("XMPP:a@b.c", &Options::gfm()),
        link("XMPP:a@b.c"),
        "should match bare address schemes case-insensitively"
    );
}

#[test]
fn bare_links_reject_phone_and_other_schemes() {
    for input in [
        "tel:+1555",
        "sms:+1555",
        "tel://a.b",
        "ssh:host",
        "ssh:/a.b",
        "sshh://a.b",
        "assh://a.b",
        "javascript://a.b",
        "data://a.b",
        "file://a.b",
    ] {
        assert_eq!(
            to_html_with_options(input, &Options::gfm()),
            format!("<p>{input}</p>"),
            "{input:?}"
        );
    }
    assert_eq!(
        to_html_with_options("1ssh://a.b", &Options::gfm()),
        "<p>1<a href=\"ssh://a.b\">ssh://a.b</a></p>",
        "should start a bare scheme after a digit"
    );
    assert_eq!(
        to_html_with_options("tel:a@b.c", &Options::gfm()),
        "<p>tel:<a href=\"mailto:a@b.c\">a@b.c</a></p>",
        "should not treat a phone scheme as an address scheme"
    );
}

#[test]
fn safe_href_keeps_every_scheme() {
    for (input, expected) in [
        ("[x](irc://a)", "<p><a href=\"irc://a\">x</a></p>"),
        ("[x](tel:+1)", "<p><a href=\"tel:+1\">x</a></p>"),
        ("[x](ssh://a)", "<p><a href=\"ssh://a\">x</a></p>"),
        ("[x](SMS:1)", "<p><a href=\"SMS:1\">x</a></p>"),
        ("[x](javascript:1)", "<p><a href=\"\">x</a></p>"),
        ("[x](vbscript:1)", "<p><a href=\"\">x</a></p>"),
        ("[x](data:text/html,x)", "<p><a href=\"\">x</a></p>"),
        ("[x](file:///etc/passwd)", "<p><a href=\"\">x</a></p>"),
    ] {
        assert_eq!(to_html(input), expected, "{input:?}");
    }
    for scheme in HOST_SCHEMES
        .iter()
        .chain(&ADDRESS_SCHEMES)
        .chain(&PHONE_SCHEMES)
    {
        assert_eq!(
            to_html(&format!("[x]({scheme}:y)")),
            format!("<p><a href=\"{scheme}:y\">x</a></p>"),
            "{scheme:?}"
        );
    }
    assert_eq!(
        to_html("![x](ssh://a)"),
        "<p><img src=\"\" alt=\"x\" /></p>",
        "should keep images to `http` and `https`"
    );
}

#[test]
fn angle_autolinks_roundtrip_for_every_scheme() {
    for destination in angle_links() {
        let input = format!("<{destination}>\n");
        for options in [ParseOptions::default(), ParseOptions::gfm()] {
            let tree = Document::parse(&input, &options).unwrap();
            let first = to_markdown(&tree).unwrap();
            assert_eq!(first, input, "{input:?}");
            let reparsed = Document::parse(&first, &options).unwrap();
            assert_eq!(canonical(&reparsed), canonical(&tree), "{input:?}");
        }
    }
    for (input, expected) in [
        ("ssh://a.b\n", "<ssh://a.b>\n"),
        ("xmpp:a@b.c\n", "<xmpp:a@b.c>\n"),
        ("tel:+1555\n", "tel:+1555\n"),
    ] {
        let options = ParseOptions::gfm();
        let tree = Document::parse(input, &options).unwrap();
        let first = to_markdown(&tree).unwrap();
        assert_eq!(first, expected, "{input:?}");
        let reparsed = Document::parse(&first, &options).unwrap();
        assert_eq!(canonical(&reparsed), canonical(&tree), "{input:?}");
    }
}

#[test]
fn address_schemes_take_exactly_one_address() {
    for input in [
        "<mailto:a@b.c?subject=x>",
        "<mailto:a@b.c,d@e.f>",
        "<xmpp:a@b.c/resource>",
    ] {
        assert_eq!(
            to_html(input),
            format!("<p>&lt;{}&gt;</p>", &input[1..input.len() - 1]),
            "{input:?}"
        );
    }
    assert_eq!(
        to_html_with_options("<mailto:a@b.c?subject=x>", &Options::gfm()),
        "<p>&lt;<a href=\"mailto:a@b.c\">mailto:a@b.c</a>?subject=x&gt;</p>",
        "should link only the bare address under GFM"
    );
    assert_eq!(
        to_html_with_options("xmpp:a@b.c/resource", &Options::gfm()),
        link("xmpp:a@b.c/resource"),
        "should keep the bare `xmpp:` resource"
    );
}

#[test]
fn bare_host_links_end_at_a_backslash() {
    assert_eq!(
        to_html_with_options("ssh://a.b\\c", &Options::gfm()),
        "<p><a href=\"ssh://a.b\">ssh://a.b</a>\\c</p>",
        "should end every host scheme at a backslash"
    );
}

#[test]
fn phone_autolinks_precede_namespaced_tags() {
    let options = Options {
        parse: ParseOptions {
            html_tag_namespaces: true,
            ..Default::default()
        },
        ..danger()
    };
    assert_eq!(
        to_html_with_options("a <tel:1> b", &options),
        "<p>a <a href=\"tel:1\">tel:1</a> b</p>",
        "should read an inline phone link as an autolink"
    );
    assert_eq!(
        to_html_with_options("<tel:1>", &options),
        "<tel:1>",
        "should still start an HTML block alone on a line"
    );
}
