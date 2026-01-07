# Typing Simulation Behavior Requirements

## Requirements
- Target use: when a text editor is focused (e.g., Google Docs in a browser) on Linux/Wayland, the tool mimics human typing to produce a provided final draft.
- Input: a “final draft” text that the system must end up producing in the editor.
- Content scope: plain text only (no rich-text formatting commands).
- Entry method: uses keyboard input events only to interact with the editor.
- Shortcuts: only use safe text-editing shortcuts that work in Google Docs and do not trigger browser/tab/system actions; must not use clipboard operations (no copy/cut/paste). See the Safe Shortcut Allowlist section below.
- Typing: produces text character-by-character (no instant full-text injection).
- Speed: human-like variability with an average target of ~40–60 WPM.
- Pauses: include randomized micro-pauses and occasional longer “thinking” pauses at human-like random times.
- Errors/variations: intentionally introduces occasional divergences from the final draft, including typos/spelling/grammar mistakes, wrong tense, and alternative word choices/phrasing.
- Error/variation rate: on average, a few divergences per paragraph; bursts are allowed.
- Immediate micro-edits: may revise recently typed text immediately after typing it (e.g., type “could not” and then quickly change it to “couldn’t”).
- Corrections: uses a mix of immediate and delayed fixes; fixes are performed via keyboard navigation (e.g., moving the cursor back), deletion (Backspace/Delete), and re-typing, then returning to where it left off.
- Review pass: a dedicated near-end review pass to fix remaining issues is allowed.
- Randomness: runs do not need to be deterministic/reproducible.
- Planning: precompute the full keyboard input sequence before starting playback.
- Outstanding divergence: no maximum limit; may leave multiple errors/variations unresolved before fixing them.
- Editor auto-substitution: no special handling; assumes the editor environment allows the final text to match the final draft exactly.
- Completion: final editor text must match the final draft exactly; ending cursor location is unconstrained.

## Definitions
- **Final draft**: the exact final text that must be present after the simulation completes.
- **Error**: any divergence from the final draft during the typing process.
- **Clipboard access**: any use of copy/cut/paste or reading clipboard contents (prohibited).
- **Safe shortcut**: a keyboard shortcut that performs editor-local text/cursor operations and does not invoke browser/tab/system commands.

## Safe Shortcut Allowlist

### Allowed
- Text entry: printable characters (including space).
- Newlines: `Enter` (paragraph), `Shift+Enter` (line break).
- Delete: `Backspace`, `Delete`, `Ctrl+Backspace`, `Ctrl+Delete`.
- Navigation: `Left/Right/Up/Down`, `Ctrl+Left/Right` (word), `Home/End` (line), `Ctrl+Home/End` (document).
- Selection: `Shift+{Left,Right,Up,Down,Home,End}`, `Ctrl+Shift+Left/Right`.
- Undo/redo: `Ctrl+Z` (undo), `Ctrl+Shift+Z` (redo). Optional fallback: `Ctrl+Y` (redo).

### Disallowed
- Clipboard: `Ctrl+C`, `Ctrl+X`, `Ctrl+V`, `Ctrl+Insert`, `Shift+Insert`.
- Browser/tab/system: `Ctrl+L`, `Ctrl+T`, `Ctrl+W`, `Ctrl+R`, `Ctrl+N`, `Ctrl+P`, `Ctrl+Tab`, `Ctrl+Shift+Tab`, `Alt+Left/Right`, `F5`, `Alt+Tab`, and any `Super` key combinations.
- Anything not in the Allowed list.