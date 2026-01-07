#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WordNavProfile {
    /// Current behavior; best for Chrome/Docs-like editors.
    #[default]
    Chrome,

    /// Conservative mode: only use Ctrl+Left/Right when the jump span is
    /// highly likely to behave consistently across apps/toolkits.
    Compatible,
}

/// Conservative predicate for deciding whether a Ctrl+Left/Right *jump span* is safe.
///
/// This returns true only when every character in `span` is either:
/// - an ASCII letter/digit, or
/// - an ASCII space (' ')
///
/// This intentionally rejects punctuation, quotes, apostrophes, dashes, underscores,
/// slashes, and newlines, because Ctrl+word navigation semantics vary widely across
/// editors/toolkits at those boundaries.
pub fn compatible_ctrl_span_is_safe(span: &[char]) -> bool {
    span.iter().all(|c| c.is_ascii_alphanumeric() || *c == ' ')
}

/// Conservative predicate for deciding whether a specific Ctrl+Left/Right jump is safe.
///
/// This is stricter than `compatible_ctrl_span_is_safe()`:
/// - The span traversed by the *planned* Ctrl+Arrow must be safe.
/// - Additionally, the immediate characters adjacent to the jump endpoints must be safe.
///
/// Rationale: different editors/toolkits can disagree about where Ctrl+Arrow stops when the
/// destination is adjacent to punctuation (e.g. hyphens in `mid-sentence`), even if the
/// *traversed* characters are only ASCII alphanumerics.
pub fn compatible_ctrl_jump_is_safe(buf: &[char], from: usize, to: usize) -> bool {
    let len = buf.len();
    let from = from.min(len);
    let to = to.min(len);
    if from == to {
        return true;
    }

    let start = from.min(to);
    let end = from.max(to);

    if !compatible_ctrl_span_is_safe(&buf[start..end]) {
        return false;
    }

    if start > 0 && !compatible_ctrl_span_is_safe(&buf[start - 1..start]) {
        return false;
    }

    if end < len && !compatible_ctrl_span_is_safe(&buf[end..end + 1]) {
        return false;
    }

    true
}
