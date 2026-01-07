use std::io::Write;
use std::os::fd::{AsFd, FromRawFd, IntoRawFd, OwnedFd};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use memfd::MemfdOptions;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::protocols::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use crate::protocols::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

use crate::model::{Action, KeyState, Plan};
use crate::trace::plan_console_trace;

#[derive(Debug, Default)]
struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        _event: <ZwpVirtualKeyboardManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        _event: <ZwpVirtualKeyboardV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

fn key_state_to_u32(state: KeyState) -> u32 {
    match state {
        KeyState::Released => 0,
        KeyState::Pressed => 1,
    }
}

fn make_keymap_fd(keymap: &str) -> Result<(OwnedFd, u32)> {
    let memfd = MemfdOptions::default()
        .allow_sealing(true)
        .create("drafter-xkb-keymap")
        .context("failed to create memfd for keymap")?;

    let mut file = memfd.as_file();
    file.write_all(keymap.as_bytes())?;
    file.write_all(&[0])?;

    let size = (keymap.as_bytes().len() + 1)
        .try_into()
        .map_err(|_| anyhow!("keymap too large"))?;

    let raw_fd = memfd.into_file().into_raw_fd();
    // SAFETY: raw_fd is owned (from into_raw_fd).
    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };

    Ok((owned_fd, size))
}

fn sleep_interruptible(stop: &AtomicBool, ms: u64) {
    let mut remaining = ms;
    while remaining > 0 {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let step = remaining.min(50);
        std::thread::sleep(Duration::from_millis(step));
        remaining -= step;
    }
}

fn print_trace_line(line: &str) {
    const RESET: &str = "\x1b[0m";
    const TYPING: &str = "\x1b[34m";
    const REPLACE: &str = "\x1b[33m";

    if let Some(rest) = line.strip_prefix("Typing") {
        eprintln!("{TYPING}Typing{RESET}{rest}");
    } else if let Some(rest) = line.strip_prefix("Replace") {
        eprintln!("{REPLACE}Replace{RESET}{rest}");
    } else {
        eprintln!("{line}");
    }
}

pub fn play_plan(plan: &Plan, countdown_secs: u64, trace: bool) -> Result<()> {
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

    let conn = Connection::connect_to_env().context("failed to connect to Wayland")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("failed to init Wayland registry")?;
    let qh = event_queue.handle();
    let mut state = State::default();

    let seat: wl_seat::WlSeat = globals
        .bind(&qh, 1..=7, ())
        .context("failed to bind wl_seat")?;

    let manager: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .context("zwp_virtual_keyboard_manager_v1 not available (is sway/wlroots exposing it?)")?;

    let keyboard: ZwpVirtualKeyboardV1 = manager.create_virtual_keyboard(&seat, &qh, ());

    event_queue
        .roundtrip(&mut state)
        .context("Wayland roundtrip failed")?;

    let (keymap_fd, keymap_size) = make_keymap_fd(&plan.config.keymap)?;
    keyboard.keymap(plan.config.keymap_format, keymap_fd.as_fd(), keymap_size);

    conn.flush().context("Wayland flush failed")?;

    let trace_events = trace.then(|| plan_console_trace(&plan.actions));
    let mut next_trace_event = 0usize;

    let start = Instant::now();

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
            Action::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                keyboard.modifiers(*mods_depressed, *mods_latched, *mods_locked, *group);
                conn.flush().ok();
            }
            Action::Key { keycode, state } => {
                let t = start.elapsed().as_millis();
                let time_ms: u32 = t.try_into().unwrap_or(u32::MAX);
                keyboard.key(time_ms, *keycode, key_state_to_u32(*state));
                conn.flush().ok();
            }
        }
    }

    if stop.load(Ordering::SeqCst) {
        eprintln!("Aborted. Attempting to reset modifiers...");
        keyboard.modifiers(0, 0, 0, 0);
        let t = start.elapsed().as_millis();
        let time_ms: u32 = t.try_into().unwrap_or(u32::MAX);
        keyboard.key(time_ms, crate::keyboard::KEY_LEFTSHIFT, 0);
        keyboard.key(time_ms, crate::keyboard::KEY_LEFTCTRL, 0);
        conn.flush().ok();
        return Err(anyhow!("aborted"));
    }

    // Ensure requests get sent.
    conn.flush().ok();

    Ok(())
}
