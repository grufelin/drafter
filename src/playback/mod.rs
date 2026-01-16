pub mod backends;

use anyhow::{anyhow, Result};

use crate::model::Plan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackBackend {
    Auto,
    Wayland,
    X11,
}

fn env_is_set(name: &str) -> bool {
    std::env::var_os(name)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}

fn auto_backend() -> PlaybackBackend {
    // Prefer Wayland if both are present (common in Wayland sessions with Xwayland).
    if env_is_set("WAYLAND_DISPLAY") || env_is_set("WAYLAND_SOCKET") {
        return PlaybackBackend::Wayland;
    }
    if env_is_set("DISPLAY") {
        return PlaybackBackend::X11;
    }

    // Unknown/unsupported environment.
    PlaybackBackend::Auto
}

fn backend_unavailable_message() -> String {
    let xdg_session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "".to_string());

    let mut parts = Vec::new();

    if env_is_set("WAYLAND_DISPLAY") {
        parts.push("WAYLAND_DISPLAY is set".to_string());
    }
    if env_is_set("WAYLAND_SOCKET") {
        parts.push("WAYLAND_SOCKET is set".to_string());
    }
    if env_is_set("DISPLAY") {
        parts.push("DISPLAY is set".to_string());
    }
    if !xdg_session_type.is_empty() {
        parts.push(format!("XDG_SESSION_TYPE={xdg_session_type}"));
    }

    if parts.is_empty() {
        "No display session detected (expected Wayland or X11 environment variables).".to_string()
    } else {
        format!("Detected environment: {}", parts.join(", "))
    }
}

fn require_supported_backend(selected: PlaybackBackend, resolved: PlaybackBackend) -> Result<()> {
    match resolved {
        PlaybackBackend::Wayland => Ok(()),
        PlaybackBackend::X11 => Err(anyhow!(
            "X11 backend selected/detected but is not supported yet. {details}",
            details = backend_unavailable_message()
        )),
        PlaybackBackend::Auto => Err(anyhow!(
            "No supported playback backend detected. {details} \n\
             Try running in a Wayland session, or pass --backend wayland to force it.",
            details = backend_unavailable_message()
        )),
    }
    .map_err(|err| {
        // Improve the error slightly if the user explicitly requested a backend.
        match selected {
            PlaybackBackend::Wayland => {
                anyhow!("Wayland backend selected but not available/unsupported. {err:#}")
            }
            PlaybackBackend::X11 => err,
            PlaybackBackend::Auto => err,
        }
    })
}

pub fn resolve_backend(requested: PlaybackBackend) -> Result<PlaybackBackend> {
    let resolved = match requested {
        PlaybackBackend::Auto => auto_backend(),
        other => other,
    };

    require_supported_backend(requested, resolved)?;
    Ok(resolved)
}

pub fn play_plan(
    plan: &Plan,
    countdown_secs: u64,
    trace: bool,
    seat_name: Option<&str>,
    backend: PlaybackBackend,
) -> Result<()> {
    let backend = resolve_backend(backend)?;

    match backend {
        PlaybackBackend::Wayland => {
            backends::wayland::play_plan_wayland(plan, countdown_secs, trace, seat_name)
        }
        PlaybackBackend::X11 => Err(anyhow!("X11 backend is not supported yet")),
        PlaybackBackend::Auto => Err(anyhow!("no backend resolved")),
    }
}
