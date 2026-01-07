use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStroke {
    pub keycode: u32,
    pub shift: bool,
}

// Linux evdev keycodes (see linux/input-event-codes.h)
pub const KEY_ESC: u32 = 1;

pub const KEY_1: u32 = 2;
pub const KEY_2: u32 = 3;
pub const KEY_3: u32 = 4;
pub const KEY_4: u32 = 5;
pub const KEY_5: u32 = 6;
pub const KEY_6: u32 = 7;
pub const KEY_7: u32 = 8;
pub const KEY_8: u32 = 9;
pub const KEY_9: u32 = 10;
pub const KEY_0: u32 = 11;

pub const KEY_MINUS: u32 = 12;
pub const KEY_EQUAL: u32 = 13;
pub const KEY_BACKSPACE: u32 = 14;
pub const KEY_TAB: u32 = 15;

pub const KEY_Q: u32 = 16;
pub const KEY_W: u32 = 17;
pub const KEY_E: u32 = 18;
pub const KEY_R: u32 = 19;
pub const KEY_T: u32 = 20;
pub const KEY_Y: u32 = 21;
pub const KEY_U: u32 = 22;
pub const KEY_I: u32 = 23;
pub const KEY_O: u32 = 24;
pub const KEY_P: u32 = 25;

pub const KEY_LEFTBRACE: u32 = 26;
pub const KEY_RIGHTBRACE: u32 = 27;
pub const KEY_ENTER: u32 = 28;

pub const KEY_LEFTCTRL: u32 = 29;

pub const KEY_A: u32 = 30;
pub const KEY_S: u32 = 31;
pub const KEY_D: u32 = 32;
pub const KEY_F: u32 = 33;
pub const KEY_G: u32 = 34;
pub const KEY_H: u32 = 35;
pub const KEY_J: u32 = 36;
pub const KEY_K: u32 = 37;
pub const KEY_L: u32 = 38;

pub const KEY_SEMICOLON: u32 = 39;
pub const KEY_APOSTROPHE: u32 = 40;
pub const KEY_GRAVE: u32 = 41;

pub const KEY_LEFTSHIFT: u32 = 42;

pub const KEY_BACKSLASH: u32 = 43;

pub const KEY_Z: u32 = 44;
pub const KEY_X: u32 = 45;
pub const KEY_C: u32 = 46;
pub const KEY_V: u32 = 47;
pub const KEY_B: u32 = 48;
pub const KEY_N: u32 = 49;
pub const KEY_M: u32 = 50;

pub const KEY_COMMA: u32 = 51;
pub const KEY_DOT: u32 = 52;
pub const KEY_SLASH: u32 = 53;

pub const KEY_RIGHTSHIFT: u32 = 54;

pub const KEY_LEFTALT: u32 = 56;
pub const KEY_SPACE: u32 = 57;

pub const KEY_DELETE: u32 = 111;

pub const KEY_LEFT: u32 = 105;
pub const KEY_RIGHT: u32 = 106;
pub const KEY_DOWN: u32 = 108;
pub const KEY_UP: u32 = 103;

pub const KEY_HOME: u32 = 102;
pub const KEY_END: u32 = 107;

pub fn typed_char_for_output_char(c: char) -> Option<char> {
    match c {
        '\n' => Some('\n'),
        // Tab and CR are intentionally unsupported: not in the safe allowlist.
        '\t' | '\r' => None,

        // Smart quotes are common in docs. We rely on the editor's auto-substitution
        // (e.g. Google Docs “smart quotes”) to turn these ASCII keystrokes into the
        // intended Unicode punctuation.
        '’' | '‘' => Some('\''),
        '”' | '“' => Some('"'),

        c if c.is_ascii_graphic() || c == ' ' => Some(c),
        _ => None,
    }
}

pub fn keystroke_for_output_char(c: char) -> Option<KeyStroke> {
    typed_char_for_output_char(c).and_then(char_to_keystroke)
}

pub fn is_supported_final_text(text: &str) -> bool {
    text.chars().all(|c| keystroke_for_output_char(c).is_some())
}

pub fn find_first_unsupported_char(text: &str) -> Option<(usize, char)> {
    text.char_indices()
        .find(|&(_idx, c)| keystroke_for_output_char(c).is_none())
}

pub fn char_to_keystroke(c: char) -> Option<KeyStroke> {
    let stroke = match c {
        'a' => KeyStroke {
            keycode: KEY_A,
            shift: false,
        },
        'b' => KeyStroke {
            keycode: KEY_B,
            shift: false,
        },
        'c' => KeyStroke {
            keycode: KEY_C,
            shift: false,
        },
        'd' => KeyStroke {
            keycode: KEY_D,
            shift: false,
        },
        'e' => KeyStroke {
            keycode: KEY_E,
            shift: false,
        },
        'f' => KeyStroke {
            keycode: KEY_F,
            shift: false,
        },
        'g' => KeyStroke {
            keycode: KEY_G,
            shift: false,
        },
        'h' => KeyStroke {
            keycode: KEY_H,
            shift: false,
        },
        'i' => KeyStroke {
            keycode: KEY_I,
            shift: false,
        },
        'j' => KeyStroke {
            keycode: KEY_J,
            shift: false,
        },
        'k' => KeyStroke {
            keycode: KEY_K,
            shift: false,
        },
        'l' => KeyStroke {
            keycode: KEY_L,
            shift: false,
        },
        'm' => KeyStroke {
            keycode: KEY_M,
            shift: false,
        },
        'n' => KeyStroke {
            keycode: KEY_N,
            shift: false,
        },
        'o' => KeyStroke {
            keycode: KEY_O,
            shift: false,
        },
        'p' => KeyStroke {
            keycode: KEY_P,
            shift: false,
        },
        'q' => KeyStroke {
            keycode: KEY_Q,
            shift: false,
        },
        'r' => KeyStroke {
            keycode: KEY_R,
            shift: false,
        },
        's' => KeyStroke {
            keycode: KEY_S,
            shift: false,
        },
        't' => KeyStroke {
            keycode: KEY_T,
            shift: false,
        },
        'u' => KeyStroke {
            keycode: KEY_U,
            shift: false,
        },
        'v' => KeyStroke {
            keycode: KEY_V,
            shift: false,
        },
        'w' => KeyStroke {
            keycode: KEY_W,
            shift: false,
        },
        'x' => KeyStroke {
            keycode: KEY_X,
            shift: false,
        },
        'y' => KeyStroke {
            keycode: KEY_Y,
            shift: false,
        },
        'z' => KeyStroke {
            keycode: KEY_Z,
            shift: false,
        },
        'A' => KeyStroke {
            keycode: KEY_A,
            shift: true,
        },
        'B' => KeyStroke {
            keycode: KEY_B,
            shift: true,
        },
        'C' => KeyStroke {
            keycode: KEY_C,
            shift: true,
        },
        'D' => KeyStroke {
            keycode: KEY_D,
            shift: true,
        },
        'E' => KeyStroke {
            keycode: KEY_E,
            shift: true,
        },
        'F' => KeyStroke {
            keycode: KEY_F,
            shift: true,
        },
        'G' => KeyStroke {
            keycode: KEY_G,
            shift: true,
        },
        'H' => KeyStroke {
            keycode: KEY_H,
            shift: true,
        },
        'I' => KeyStroke {
            keycode: KEY_I,
            shift: true,
        },
        'J' => KeyStroke {
            keycode: KEY_J,
            shift: true,
        },
        'K' => KeyStroke {
            keycode: KEY_K,
            shift: true,
        },
        'L' => KeyStroke {
            keycode: KEY_L,
            shift: true,
        },
        'M' => KeyStroke {
            keycode: KEY_M,
            shift: true,
        },
        'N' => KeyStroke {
            keycode: KEY_N,
            shift: true,
        },
        'O' => KeyStroke {
            keycode: KEY_O,
            shift: true,
        },
        'P' => KeyStroke {
            keycode: KEY_P,
            shift: true,
        },
        'Q' => KeyStroke {
            keycode: KEY_Q,
            shift: true,
        },
        'R' => KeyStroke {
            keycode: KEY_R,
            shift: true,
        },
        'S' => KeyStroke {
            keycode: KEY_S,
            shift: true,
        },
        'T' => KeyStroke {
            keycode: KEY_T,
            shift: true,
        },
        'U' => KeyStroke {
            keycode: KEY_U,
            shift: true,
        },
        'V' => KeyStroke {
            keycode: KEY_V,
            shift: true,
        },
        'W' => KeyStroke {
            keycode: KEY_W,
            shift: true,
        },
        'X' => KeyStroke {
            keycode: KEY_X,
            shift: true,
        },
        'Y' => KeyStroke {
            keycode: KEY_Y,
            shift: true,
        },
        'Z' => KeyStroke {
            keycode: KEY_Z,
            shift: true,
        },
        '1' => KeyStroke {
            keycode: KEY_1,
            shift: false,
        },
        '2' => KeyStroke {
            keycode: KEY_2,
            shift: false,
        },
        '3' => KeyStroke {
            keycode: KEY_3,
            shift: false,
        },
        '4' => KeyStroke {
            keycode: KEY_4,
            shift: false,
        },
        '5' => KeyStroke {
            keycode: KEY_5,
            shift: false,
        },
        '6' => KeyStroke {
            keycode: KEY_6,
            shift: false,
        },
        '7' => KeyStroke {
            keycode: KEY_7,
            shift: false,
        },
        '8' => KeyStroke {
            keycode: KEY_8,
            shift: false,
        },
        '9' => KeyStroke {
            keycode: KEY_9,
            shift: false,
        },
        '0' => KeyStroke {
            keycode: KEY_0,
            shift: false,
        },
        '!' => KeyStroke {
            keycode: KEY_1,
            shift: true,
        },
        '@' => KeyStroke {
            keycode: KEY_2,
            shift: true,
        },
        '#' => KeyStroke {
            keycode: KEY_3,
            shift: true,
        },
        '$' => KeyStroke {
            keycode: KEY_4,
            shift: true,
        },
        '%' => KeyStroke {
            keycode: KEY_5,
            shift: true,
        },
        '^' => KeyStroke {
            keycode: KEY_6,
            shift: true,
        },
        '&' => KeyStroke {
            keycode: KEY_7,
            shift: true,
        },
        '*' => KeyStroke {
            keycode: KEY_8,
            shift: true,
        },
        '(' => KeyStroke {
            keycode: KEY_9,
            shift: true,
        },
        ')' => KeyStroke {
            keycode: KEY_0,
            shift: true,
        },
        '-' => KeyStroke {
            keycode: KEY_MINUS,
            shift: false,
        },
        '_' => KeyStroke {
            keycode: KEY_MINUS,
            shift: true,
        },
        '=' => KeyStroke {
            keycode: KEY_EQUAL,
            shift: false,
        },
        '+' => KeyStroke {
            keycode: KEY_EQUAL,
            shift: true,
        },
        '[' => KeyStroke {
            keycode: KEY_LEFTBRACE,
            shift: false,
        },
        '{' => KeyStroke {
            keycode: KEY_LEFTBRACE,
            shift: true,
        },
        ']' => KeyStroke {
            keycode: KEY_RIGHTBRACE,
            shift: false,
        },
        '}' => KeyStroke {
            keycode: KEY_RIGHTBRACE,
            shift: true,
        },
        '\\' => KeyStroke {
            keycode: KEY_BACKSLASH,
            shift: false,
        },
        '|' => KeyStroke {
            keycode: KEY_BACKSLASH,
            shift: true,
        },
        ';' => KeyStroke {
            keycode: KEY_SEMICOLON,
            shift: false,
        },
        ':' => KeyStroke {
            keycode: KEY_SEMICOLON,
            shift: true,
        },
        '\'' => KeyStroke {
            keycode: KEY_APOSTROPHE,
            shift: false,
        },
        '"' => KeyStroke {
            keycode: KEY_APOSTROPHE,
            shift: true,
        },
        '`' => KeyStroke {
            keycode: KEY_GRAVE,
            shift: false,
        },
        '~' => KeyStroke {
            keycode: KEY_GRAVE,
            shift: true,
        },
        ',' => KeyStroke {
            keycode: KEY_COMMA,
            shift: false,
        },
        '<' => KeyStroke {
            keycode: KEY_COMMA,
            shift: true,
        },
        '.' => KeyStroke {
            keycode: KEY_DOT,
            shift: false,
        },
        '>' => KeyStroke {
            keycode: KEY_DOT,
            shift: true,
        },
        '/' => KeyStroke {
            keycode: KEY_SLASH,
            shift: false,
        },
        '?' => KeyStroke {
            keycode: KEY_SLASH,
            shift: true,
        },
        ' ' => KeyStroke {
            keycode: KEY_SPACE,
            shift: false,
        },
        '\n' => KeyStroke {
            keycode: KEY_ENTER,
            shift: false,
        },
        _ => return None,
    };
    Some(stroke)
}

pub fn qwerty_adjacent_char(c: char, rng: &mut impl Rng) -> Option<char> {
    let (base, make_upper) = if c.is_ascii_uppercase() {
        (c.to_ascii_lowercase(), true)
    } else {
        (c, false)
    };

    let neighbors: &[char] = match base {
        'a' => &['q', 'w', 's', 'z', 'x'],
        'b' => &['v', 'g', 'h', 'n'],
        'c' => &['x', 'd', 'f', 'v'],
        'd' => &['s', 'e', 'r', 'f', 'c', 'x'],
        'e' => &['w', 's', 'd', 'r'],
        'f' => &['d', 'r', 't', 'g', 'v', 'c'],
        'g' => &['f', 't', 'y', 'h', 'b', 'v'],
        'h' => &['g', 'y', 'u', 'j', 'n', 'b'],
        'i' => &['u', 'j', 'k', 'o'],
        'j' => &['h', 'u', 'i', 'k', 'm', 'n'],
        'k' => &['j', 'i', 'o', 'l', ',', 'm'],
        'l' => &['k', 'o', 'p', ';', '.'],
        'm' => &['n', 'j', 'k', ','],
        'n' => &['b', 'h', 'j', 'm'],
        'o' => &['i', 'k', 'l', 'p'],
        'p' => &['o', 'l', '['],
        'q' => &['w', 'a'],
        'r' => &['e', 'd', 'f', 't'],
        's' => &['a', 'w', 'e', 'd', 'x', 'z'],
        't' => &['r', 'f', 'g', 'y'],
        'u' => &['y', 'h', 'j', 'i'],
        'v' => &['c', 'f', 'g', 'b'],
        'w' => &['q', 'a', 's', 'e'],
        'x' => &['z', 's', 'd', 'c'],
        'y' => &['t', 'g', 'h', 'u'],
        'z' => &['a', 's', 'x'],
        '1' => &['2', 'q'],
        '2' => &['1', '3', 'q', 'w'],
        '3' => &['2', '4', 'w', 'e'],
        '4' => &['3', '5', 'e', 'r'],
        '5' => &['4', '6', 'r', 't'],
        '6' => &['5', '7', 't', 'y'],
        '7' => &['6', '8', 'y', 'u'],
        '8' => &['7', '9', 'u', 'i'],
        '9' => &['8', '0', 'i', 'o'],
        '0' => &['9', 'o', 'p'],
        _ => return None,
    };

    let chosen = neighbors[rng.gen_range(0..neighbors.len())];
    Some(if make_upper {
        chosen.to_ascii_uppercase()
    } else {
        chosen
    })
}
