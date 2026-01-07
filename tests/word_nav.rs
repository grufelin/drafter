use drafter::word_nav::{ctrl_left, ctrl_right};

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '\''
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

#[test]
fn ctrl_left_moves_to_word_start_and_skips_whitespace() {
    let buf = chars("hello   world");

    assert_eq!(ctrl_left(&buf, 3, is_word_char), 0);
    assert_eq!(ctrl_left(&buf, 6, is_word_char), 0);
    assert_eq!(ctrl_left(&buf, 8, is_word_char), 0);
}

#[test]
fn ctrl_right_moves_to_word_end_or_next_word_start() {
    let buf = chars("hello   world");

    assert_eq!(ctrl_right(&buf, 0, is_word_char), 5);
    assert_eq!(ctrl_right(&buf, 5, is_word_char), 8);
    assert_eq!(ctrl_right(&buf, 6, is_word_char), 8);
    assert_eq!(ctrl_right(&buf, 8, is_word_char), buf.len());
}

#[test]
fn ctrl_word_nav_treats_punctuation_as_its_own_run() {
    let buf = chars("hello...world");

    let punctuation_start = 5;
    let world_start = 8;

    assert_eq!(ctrl_left(&buf, buf.len(), is_word_char), world_start);
    assert_eq!(
        ctrl_left(&buf, world_start, is_word_char),
        punctuation_start
    );

    assert_eq!(
        ctrl_right(&buf, punctuation_start, is_word_char),
        world_start
    );
    assert_eq!(ctrl_right(&buf, world_start, is_word_char), buf.len());
}
