use drafter::llm::{validate_phrase_alternatives, PhraseAlternative};

#[test]
fn validate_accepts_unique_non_overlapping_alternatives() {
    let paragraph = "The meeting dragged on, and by the end everyone looked tired.";
    let items = vec![
        PhraseAlternative {
            original: "dragged on, and by the end everyone".to_string(),
            alternative: "ran long. Everyone".to_string(),
        },
        PhraseAlternative {
            original: "looked tired".to_string(),
            alternative: "looked tired by the end".to_string(),
        },
    ];

    validate_phrase_alternatives(paragraph, &items).expect("should validate");
}

#[test]
fn validate_rejects_non_unique_original() {
    let paragraph = "word word";
    let items = vec![PhraseAlternative {
        original: "word".to_string(),
        alternative: "term".to_string(),
    }];

    let err = validate_phrase_alternatives(paragraph, &items).unwrap_err();
    assert!(
        err.to_string().contains("exactly once"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn validate_rejects_overlapping_originals() {
    let paragraph = "abcde";
    let items = vec![
        PhraseAlternative {
            original: "abc".to_string(),
            alternative: "abx".to_string(),
        },
        PhraseAlternative {
            original: "bcd".to_string(),
            alternative: "bxd".to_string(),
        },
    ];

    let err = validate_phrase_alternatives(paragraph, &items).unwrap_err();
    assert!(
        err.to_string().contains("non-overlapping"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn validate_rejects_whitespace_at_span_edges() {
    let paragraph = "hello world";
    let items = vec![PhraseAlternative {
        original: "hello ".to_string(),
        alternative: "hi".to_string(),
    }];

    let err = validate_phrase_alternatives(paragraph, &items).unwrap_err();
    assert!(
        err.to_string().contains("whitespace"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn validate_rejects_unsupported_chars() {
    let paragraph = "hello world";
    let items = vec![PhraseAlternative {
        original: "world".to_string(),
        alternative: "wo\trld".to_string(),
    }];

    let err = validate_phrase_alternatives(paragraph, &items).unwrap_err();
    assert!(
        err.to_string().contains("unsupported"),
        "unexpected error: {err:?}"
    );
}
