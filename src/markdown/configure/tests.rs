// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::Options;

#[test]
fn validates_markdown_serialization_markers() {
    let options = Options::default();
    assert_eq!(options.check_bullet().unwrap(), '-');
    assert_eq!(options.check_bullet_ordered().unwrap(), '.');
    assert_eq!(options.check_bullet_other().unwrap(), '*');
    assert_eq!(options.check_emphasis().unwrap(), '*');
    assert_eq!(options.check_fence().unwrap(), '`');
    assert_eq!(options.check_quote().unwrap(), '"');
    assert_eq!(options.check_rule().unwrap(), '*');
    assert_eq!(options.check_rule_repetition().unwrap(), 3);
    assert_eq!(options.check_strong().unwrap(), '*');

    for bullet in ['+', '-'] {
        let options = Options {
            bullet,
            ..Options::default()
        };
        assert_eq!(options.check_bullet().unwrap(), bullet);
    }

    for bullet in ['.', ')'] {
        let options = Options {
            bullet_ordered: bullet,
            ..Options::default()
        };
        assert_eq!(options.check_bullet_ordered().unwrap(), bullet);
    }

    let options = Options {
        bullet: '+',
        bullet_other: '-',
        ..Options::default()
    };
    assert_eq!(options.check_bullet_other().unwrap(), '*');

    for marker in ['_', '*'] {
        let options = Options {
            emphasis: marker,
            strong: marker,
            ..Options::default()
        };
        assert_eq!(options.check_emphasis().unwrap(), marker);
        assert_eq!(options.check_strong().unwrap(), marker);
    }

    let options = Options {
        fence: '~',
        quote: '\'',
        rule: '-',
        rule_repetition: 4,
        ..Options::default()
    };
    assert_eq!(options.check_fence().unwrap(), '~');
    assert_eq!(options.check_quote().unwrap(), '\'');
    assert_eq!(options.check_rule().unwrap(), '-');
    assert_eq!(options.check_rule_repetition().unwrap(), 4);

    let invalid = Options {
        bullet: 'x',
        ..Options::default()
    };
    assert!(invalid.check_bullet().is_err());

    let invalid = Options {
        bullet_ordered: 'x',
        ..Options::default()
    };
    assert!(invalid.check_bullet_ordered().is_err());

    let invalid = Options {
        bullet: '*',
        bullet_other: 'x',
        ..Options::default()
    };
    assert!(invalid.check_bullet_other().is_err());

    let invalid = Options {
        bullet: 'x',
        bullet_other: '-',
        ..Options::default()
    };
    assert!(invalid.check_bullet_other().is_err());

    let invalid = Options {
        bullet: '*',
        bullet_other: '*',
        ..Options::default()
    };
    assert!(invalid.check_bullet_other().is_err());

    for options in [
        Options {
            emphasis: 'x',
            ..Options::default()
        },
        Options {
            strong: 'x',
            ..Options::default()
        },
        Options {
            fence: 'x',
            ..Options::default()
        },
        Options {
            quote: 'x',
            ..Options::default()
        },
        Options {
            rule: 'x',
            ..Options::default()
        },
        Options {
            rule_repetition: 2,
            ..Options::default()
        },
    ] {
        assert!(options.check_emphasis().is_err() || options.emphasis == '*');
        assert!(options.check_strong().is_err() || options.strong == '*');
        assert!(options.check_fence().is_err() || options.fence == '`');
        assert!(options.check_quote().is_err() || options.quote == '"');
        assert!(options.check_rule().is_err() || options.rule == '*');
        assert!(options.check_rule_repetition().is_err() || options.rule_repetition == 3);
    }
}
