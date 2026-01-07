use drafter::planner::PlannerConfig;
use drafter::word_nav_profile::{
    compatible_ctrl_jump_is_safe, compatible_ctrl_span_is_safe, WordNavProfile,
};

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

#[test]
fn default_planner_config_uses_chrome_word_nav_profile() {
    assert_eq!(
        PlannerConfig::default().word_nav_profile,
        WordNavProfile::Chrome
    );
}

#[test]
fn compatible_safe_span_allows_ascii_words_and_spaces_only() {
    assert!(compatible_ctrl_span_is_safe(&chars("hello world 123")));
    assert!(compatible_ctrl_span_is_safe(&chars("A B C 9")));
    assert!(compatible_ctrl_span_is_safe(&[]));
}

#[test]
fn compatible_ctrl_jump_rejects_alnum_span_adjacent_to_punctuation() {
    let buf = chars("mid-sentence");
    assert!(!compatible_ctrl_jump_is_safe(&buf, buf.len(), 4));

    let buf = chars("hello,world");
    assert!(!compatible_ctrl_jump_is_safe(&buf, buf.len(), 6));

    let buf = chars("hello world");
    assert!(compatible_ctrl_jump_is_safe(&buf, buf.len(), 6));
}

#[test]
fn compatible_safe_span_rejects_risky_characters() {
    let cases = [
        "don't",
        "don’t",
        "mid-sentence",
        "Wait—really",
        "Wait–really",
        "\"why\"",
        "“why”",
        "…",
        "...",
        "hello,world",
        "hello.world",
        "(x)",
        "[x]",
        "{x}",
        "a/b",
        "a\\b",
        "hello_world",
        "hello\nworld",
        "hello\tworld",
    ];

    for s in cases {
        assert!(
            !compatible_ctrl_span_is_safe(&chars(s)),
            "expected reject: {s:?}"
        );
    }
}
