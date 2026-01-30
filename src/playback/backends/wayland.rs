use std::collections::HashMap;
use std::io::Write;
use std::os::fd::{AsFd, FromRawFd, IntoRawFd, OwnedFd};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use memfd::MemfdOptions;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::model::{Action, KeyState, Plan};
use crate::playback::util::{print_trace_line, sleep_interruptible};
use crate::protocols::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use crate::protocols::virtual_keyboard_unstable_v1::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;
use crate::trace::plan_console_trace;

#[derive(Debug, Clone)]
struct SeatData {
    global_name: u32,
}

#[derive(Debug, Default)]
struct State {
    seat_names_by_global: HashMap<u32, String>,
}

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

impl Dispatch<wl_seat::WlSeat, SeatData> for State {
    fn event(
        state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        event: wl_seat::Event,
        data: &SeatData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Name { name } = event {
            state.seat_names_by_global.insert(data.global_name, name);
        }
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
    let owned_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };

    Ok((owned_fd, size))
}

pub fn play_plan_wayland(
    plan: &Plan,
    countdown_secs: u64,
    trace: bool,
    seat_name: Option<&str>,
) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    {
        let stop = stop.clone();
        ctrlc::set_handler(move || {
            stop.store(true, Ordering::SeqCst);
        })
        .context("failed to install Ctrl+C handler")?;
    }

    let conn = Connection::connect_to_env().context("failed to connect to Wayland")?;
    let (globals, mut event_queue) =
        registry_queue_init(&conn).context("failed to init Wayland registry")?;
    let qh = event_queue.handle();
    let mut state = State::default();

    let manager: ZwpVirtualKeyboardManagerV1 = globals
        .bind(&qh, 1..=1, ())
        .context("zwp_virtual_keyboard_manager_v1 not available (is sway/wlroots exposing it?)")?;

    let seat_globals: Vec<_> = globals
        .contents()
        .clone_list()
        .into_iter()
        .filter(|g| g.interface == wl_seat::WlSeat::interface().name)
        .collect();

    if seat_globals.is_empty() {
        return Err(anyhow!("wl_seat not available (no seats advertised)"));
    }

    let seat: wl_seat::WlSeat = match seat_name {
        Some(requested) => {
            let mut seats = Vec::with_capacity(seat_globals.len());
            for g in seat_globals.iter() {
                let version = g.version.min(7);
                let seat: wl_seat::WlSeat = globals.registry().bind(
                    g.name,
                    version,
                    &qh,
                    SeatData {
                        global_name: g.name,
                    },
                );
                seats.push((g.name, seat));
            }

            event_queue
                .roundtrip(&mut state)
                .context("Wayland roundtrip (seat discovery) failed")?;

            if let Some(seat) = seats.iter().find_map(|(global_name, seat)| {
                state
                    .seat_names_by_global
                    .get(global_name)
                    .filter(|n| n.as_str() == requested)
                    .map(|_| seat.clone())
            }) {
                seat
            } else {
                let mut names = state
                    .seat_names_by_global
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                names.sort();
                names.dedup();

                if names.is_empty() {
                    return Err(anyhow!(
                        "requested seat {requested:?}, but compositor did not advertise any wl_seat.name values (requires wl_seat v2+)"
                    ));
                }

                return Err(anyhow!(
                    "requested seat {requested:?} not found; available seats: {}",
                    names.join(", ")
                ));
            }
        }
        None => {
            let g = &seat_globals[0];
            let version = g.version.min(7);
            globals.registry().bind(
                g.name,
                version,
                &qh,
                SeatData {
                    global_name: g.name,
                },
            )
        }
    };

    let keyboard: ZwpVirtualKeyboardV1 = manager.create_virtual_keyboard(&seat, &qh, ());

    event_queue
        .roundtrip(&mut state)
        .context("Wayland roundtrip failed")?;

    let (keymap_fd, keymap_size) = make_keymap_fd(&plan.config.keymap)?;
    keyboard.keymap(plan.config.keymap_format, keymap_fd.as_fd(), keymap_size);

    conn.flush().context("Wayland flush failed")?;

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

    let trace_events = trace.then(|| plan_console_trace(&plan.actions));
    let mut next_trace_event = 0usize;

    let start = Instant::now();

    let reset_modifiers_best_effort = |keyboard: &ZwpVirtualKeyboardV1| {
        keyboard.modifiers(0, 0, 0, 0);

        let t = start.elapsed().as_millis();
        let time_ms: u32 = t.try_into().unwrap_or(u32::MAX);

        // Best-effort releases. We may send releases even if not down; this is intended to
        // reduce the chance of leaving stuck modifiers if playback is aborted mid-run.
        for keycode in super::COMMON_MODIFIER_KEYCODES {
            keyboard.key(time_ms, keycode, 0);
        }
    };

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
                if let Err(e) = conn.flush().with_context(|| {
                    format!("Wayland flush failed (action_index={action_index}, action=modifiers)")
                }) {
                    eprintln!("Playback error. Attempting to reset modifiers...");
                    reset_modifiers_best_effort(&keyboard);
                    let _ = conn.flush();
                    return Err(e);
                }
            }
            Action::Key { keycode, state } => {
                let t = start.elapsed().as_millis();
                let time_ms: u32 = t.try_into().unwrap_or(u32::MAX);
                keyboard.key(time_ms, *keycode, key_state_to_u32(*state));
                if let Err(e) = conn.flush().with_context(|| {
                    format!(
                        "Wayland flush failed (action_index={action_index}, action=key keycode={keycode} state={state:?})"
                    )
                }) {
                    eprintln!("Playback error. Attempting to reset modifiers...");
                    reset_modifiers_best_effort(&keyboard);
                    let _ = conn.flush();
                    return Err(e);
                }
            }
        }
    }

    if stop.load(Ordering::SeqCst) {
        eprintln!("Aborted. Attempting to reset modifiers...");
        reset_modifiers_best_effort(&keyboard);
        conn.flush().ok();
        return Err(anyhow!("aborted"));
    }

    conn.flush().ok();

    Ok(())
}
