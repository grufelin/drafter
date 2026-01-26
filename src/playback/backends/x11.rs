use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, GetInputFocusReply};
use x11rb::protocol::xtest::ConnectionExt as _;
use x11rb::protocol::{xproto, xtest};

use crate::model::{Action, KeyState, Plan};
use crate::playback::util::{print_trace_line, sleep_interruptible};
use crate::trace::plan_console_trace;

fn evdev_to_x11_keycode(evdev_keycode: u32) -> Result<u8> {
    // On most Linux Xorg setups, X11 keycodes are evdev + 8.
    // evdev keycodes in this project are small (< 256); clamp defensively.
    let x11 = evdev_keycode
        .checked_add(8)
        .ok_or_else(|| anyhow!("evdev keycode overflow"))?;
    u8::try_from(x11).map_err(|_| anyhow!("evdev keycode {evdev_keycode} out of range for X11"))
}

fn key_state_to_x11_event_type(state: KeyState) -> u8 {
    match state {
        KeyState::Pressed => xproto::KEY_PRESS_EVENT,
        KeyState::Released => xproto::KEY_RELEASE_EVENT,
    }
}

fn query_xtest(conn: &impl Connection) -> Result<()> {
    let ext = conn
        .extension_information(xtest::X11_EXTENSION_NAME)
        .context("failed to query X11 extension info")?;

    if ext.is_none() {
        return Err(anyhow!(
            "X11 backend requires the XTEST extension (not present on this X server)"
        ));
    }

    // Optional sanity check: ask for a version. If this fails, we still treat it as unsupported.
    let _ = conn
        .xtest_get_version(2, 2)
        .ok()
        .and_then(|cookie| cookie.reply().ok());

    Ok(())
}

fn get_focus(conn: &impl Connection) -> Result<GetInputFocusReply> {
    conn.get_input_focus()
        .context("failed to request input focus")?
        .reply()
        .context("failed to read input focus reply")
}

fn keysym_for_keycode(conn: &impl Connection, keycode: u8, index: usize) -> Result<xproto::Keysym> {
    let reply = conn
        .get_keyboard_mapping(keycode, 1)
        .context("failed to request keyboard mapping")?
        .reply()
        .context("failed to read keyboard mapping")?;

    let per = reply.keysyms_per_keycode as usize;
    if per == 0 {
        return Err(anyhow!("X server returned 0 keysyms per keycode"));
    }

    Ok(reply
        .keysyms
        .get(index)
        .copied()
        .unwrap_or(x11rb::NO_SYMBOL))
}

fn latin1_keysym(c: char) -> xproto::Keysym {
    // For Latin-1, X11 keysyms match the character code.
    // (ASCII is a subset of Latin-1, which is all we need for US layout checks.)
    c as u32
}

fn validate_us_keymap(conn: &impl Connection) -> Result<()> {
    // We only support US QWERTY for now.
    // Validate using a small set of representative keys.
    let checks: &[(u32, xproto::Keysym, xproto::Keysym)] = &[
        // (evdev, unshifted keysym, shifted keysym)
        (
            crate::keyboard::KEY_A,
            latin1_keysym('a'),
            latin1_keysym('A'),
        ),
        (
            crate::keyboard::KEY_Q,
            latin1_keysym('q'),
            latin1_keysym('Q'),
        ),
        (
            crate::keyboard::KEY_1,
            latin1_keysym('1'),
            latin1_keysym('!'),
        ),
        (
            crate::keyboard::KEY_MINUS,
            latin1_keysym('-'),
            latin1_keysym('_'),
        ),
        (
            crate::keyboard::KEY_APOSTROPHE,
            latin1_keysym('\''),
            latin1_keysym('"'),
        ),
        (
            crate::keyboard::KEY_LEFTBRACE,
            latin1_keysym('['),
            latin1_keysym('{'),
        ),
        (
            crate::keyboard::KEY_RIGHTBRACE,
            latin1_keysym(']'),
            latin1_keysym('}'),
        ),
    ];

    let mut no_symbol_count = 0usize;
    let mut first_no_symbol: Option<(u8, xproto::Keysym, xproto::Keysym)> = None;
    let mut first_mismatch: Option<(u8, xproto::Keysym, xproto::Keysym)> = None;

    for (evdev, unshifted, shifted) in checks {
        let keycode = evdev_to_x11_keycode(*evdev)?;
        // We assume index 0 is unshifted, index 1 is shifted.
        let got0 = keysym_for_keycode(conn, keycode, 0)?;
        let got1 = keysym_for_keycode(conn, keycode, 1)?;

        if got0 == x11rb::NO_SYMBOL || got1 == x11rb::NO_SYMBOL {
            no_symbol_count += 1;
            if first_no_symbol.is_none() {
                first_no_symbol = Some((keycode, got0, got1));
            }
            continue;
        }

        if (got0 != *unshifted || got1 != *shifted) && first_mismatch.is_none() {
            first_mismatch = Some((keycode, got0, got1));
        }
    }

    if no_symbol_count > 0 {
        let extra = if let Some((keycode, got0, got1)) = first_no_symbol {
            format!(" (example keycode {keycode}: got {got0:#x}/{got1:#x})")
        } else {
            String::new()
        };

        return Err(anyhow!(
            "X11 backend could not validate the X server keymap because some representative keys returned NoSymbol{extra}. This backend assumes X11 keycodes are evdev+8 and currently requires a US keymap; unusual server keycode mappings may not work."
        ));
    }

    if let Some((keycode, got0, got1)) = first_mismatch {
        return Err(anyhow!(
            "X11 backend currently requires a US keyboard layout, but the X server keymap does not match (keycode {keycode}: got {got0:#x}/{got1:#x}). Try `setxkbmap us`."
        ));
    }

    Ok(())
}

fn xtest_key(
    conn: &impl Connection,
    root: xproto::Window,
    keycode: u8,
    state: KeyState,
) -> Result<()> {
    // XTEST FakeInput wants:
    // - type_: KeyPress/KeyRelease
    // - detail: keycode
    // - time: CURRENT_TIME
    // - root/root_x/root_y: used for pointer-related events; still required
    // - deviceid: 0 (core keyboard)
    let type_ = key_state_to_x11_event_type(state);
    conn.xtest_fake_input(type_, keycode, x11rb::CURRENT_TIME, root, 0, 0, 0)
        .context("failed to send XTEST fake input")?;
    Ok(())
}

fn reset_common_modifiers_best_effort(conn: &impl Connection, root: xproto::Window) {
    // Best-effort release. We may send releases even if not down; this is intended to
    // avoid leaving a stuck modifier (or starting with one) when a previous run was aborted.
    for keycode in [
        crate::keyboard::KEY_LEFTSHIFT,
        crate::keyboard::KEY_RIGHTSHIFT,
        crate::keyboard::KEY_LEFTCTRL,
        crate::keyboard::KEY_RIGHTCTRL,
        crate::keyboard::KEY_LEFTALT,
    ] {
        if let Ok(code) = evdev_to_x11_keycode(keycode) {
            let _ = xtest_key(conn, root, code, KeyState::Released);
        }
    }
    let _ = conn.flush();
}

pub fn play_plan_x11(plan: &Plan, countdown_secs: u64, trace: bool) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
        })
        .context("failed to install Ctrl+C handler")?;
    }

    if countdown_secs > 0 {
        eprintln!("Focus the target editor window. Starting in {countdown_secs}s...");
        for remaining in (1..=countdown_secs).rev() {
            if stop.load(Ordering::SeqCst) {
                return Err(anyhow!("aborted"));
            }
            eprintln!("{remaining}...");
            sleep_interruptible(stop.as_ref(), 1000);
        }
        if stop.load(Ordering::SeqCst) {
            return Err(anyhow!("aborted"));
        }
    }

    let (conn, screen_num) = x11rb::connect(None).context("failed to connect to X11")?;
    query_xtest(&conn)?;
    validate_us_keymap(&conn)?;

    let setup = conn.setup();
    let screen = setup
        .roots
        .get(screen_num)
        .ok_or_else(|| anyhow!("invalid X11 screen index"))?;

    // Sanity check: require explicit input focus.
    let focus = get_focus(&conn)?;
    // X11 special focus value: PointerRoot means the focused window follows the pointer.
    // (`focus` is a `Window` newtype in the protocol, but x11rb models it as `u32`.)
    const POINTER_ROOT: xproto::Window = 1;
    if focus.focus == x11rb::NONE {
        return Err(anyhow!(
            "no X11 input focus detected; click into the target editor before starting"
        ));
    }
    if focus.focus == POINTER_ROOT {
        return Err(anyhow!(
            "X11 input focus is set to PointerRoot; click into the target editor window to give it explicit focus before starting"
        ));
    }

    // Unlike Wayland, X11 has no way to set per-client modifier state. Reset common modifiers
    // to try to start from a neutral state (e.g. if a previous run was aborted).
    reset_common_modifiers_best_effort(&conn, screen.root);

    let trace_events = trace.then(|| plan_console_trace(&plan.actions));
    let mut next_trace_event = 0usize;

    for (action_index, action) in plan.actions.iter().enumerate() {
        if stop.load(Ordering::SeqCst) {
            break;
        }

        if let Some(events) = &trace_events {
            while next_trace_event < events.len()
                && events[next_trace_event].action_index == action_index
            {
                print_trace_line(&events[next_trace_event].line);
                next_trace_event += 1;
            }
        }

        match action {
            Action::Wait { ms } => {
                sleep_interruptible(stop.as_ref(), *ms);
            }
            Action::Modifiers { .. } => {
                // No-op on X11. We rely on explicit modifier key presses/releases.
            }
            Action::Key { keycode, state } => {
                let x11_keycode = evdev_to_x11_keycode(*keycode)?;

                // Note: we don't attempt to set timestamps; XTEST supports CURRENT_TIME.
                xtest_key(&conn, screen.root, x11_keycode, *state)?;
                conn.flush().context("failed to flush X11 connection")?;
            }
        }
    }

    if stop.load(Ordering::SeqCst) {
        eprintln!("Aborted. Attempting to reset modifiers...");

        reset_common_modifiers_best_effort(&conn, screen.root);

        return Err(anyhow!("aborted"));
    }

    conn.flush().context("failed to flush X11 connection")?;
    Ok(())
}
