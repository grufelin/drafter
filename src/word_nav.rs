#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Whitespace,
    Punctuation,
}

fn classify_char(is_word_char: impl Fn(char) -> bool, c: char) -> CharClass {
    if c.is_ascii_whitespace() {
        CharClass::Whitespace
    } else if is_word_char(c) {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

/// Compute the new cursor position for Ctrl+Left word navigation.
///
/// This is intentionally editor-agnostic and based on character classes:
///
/// - word characters (as defined by `is_word_char`)
/// - whitespace (`is_ascii_whitespace`)
/// - punctuation/other (everything else)
///
/// Behavior:
/// - If the cursor is in whitespace, skip the whole whitespace run first.
/// - Then move left over the contiguous run of word or punctuation characters.
/// - Stop at the beginning of that run.
pub fn ctrl_left(buf: &[char], cursor: usize, is_word_char: impl Fn(char) -> bool) -> usize {
    let mut idx = cursor.min(buf.len());
    if idx == 0 {
        return 0;
    }

    while idx > 0 && classify_char(&is_word_char, buf[idx - 1]) == CharClass::Whitespace {
        idx -= 1;
    }

    if idx == 0 {
        return 0;
    }

    let class = classify_char(&is_word_char, buf[idx - 1]);
    debug_assert!(class != CharClass::Whitespace);

    while idx > 0 && classify_char(&is_word_char, buf[idx - 1]) == class {
        idx -= 1;
    }

    idx
}

/// Compute the new cursor position for Ctrl+Right word navigation.
///
/// See `ctrl_left` for the character-class model.
///
/// Behavior:
/// - If the cursor is in whitespace, skip the whole whitespace run first and stop at the
///   start of the next word/punctuation run.
/// - Otherwise, move right over the contiguous run of word or punctuation characters and
///   stop at the end of that run.
pub fn ctrl_right(buf: &[char], cursor: usize, is_word_char: impl Fn(char) -> bool) -> usize {
    let mut idx = cursor.min(buf.len());
    if idx >= buf.len() {
        return buf.len();
    }

    let started_in_whitespace = classify_char(&is_word_char, buf[idx]) == CharClass::Whitespace;

    while idx < buf.len() && classify_char(&is_word_char, buf[idx]) == CharClass::Whitespace {
        idx += 1;
    }

    if idx >= buf.len() {
        return buf.len();
    }

    if started_in_whitespace {
        return idx;
    }

    let class = classify_char(&is_word_char, buf[idx]);
    debug_assert!(class != CharClass::Whitespace);

    while idx < buf.len() && classify_char(&is_word_char, buf[idx]) == class {
        idx += 1;
    }

    idx
}
