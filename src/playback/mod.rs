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
    if cfg!(feature = "wayland") && (env_is_set("WAYLAND_DISPLAY") || env_is_set("WAYLAND_SOCKET"))
    {
        return PlaybackBackend::Wayland;
    }
    if cfg!(feature = "x11") && env_is_set("DISPLAY") {
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
        PlaybackBackend::Wayland => {
            #[cfg(feature = "wayland")]
            {
                Ok(())
            }

            #[cfg(not(feature = "wayland"))]
            {
                Err(anyhow!(
                    "Wayland backend selected/detected but is disabled in this build. (Rebuild with `--features wayland`.) {details}",
                    details = backend_unavailable_message()
                ))
            }
        }
        PlaybackBackend::X11 => {
            #[cfg(feature = "x11")]
            {
                Ok(())
            }

            #[cfg(not(feature = "x11"))]
            {
                Err(anyhow!(
                    "X11 backend selected/detected but is disabled in this build. (Rebuild with `--features x11`.) {details}",
                    details = backend_unavailable_message()
                ))
            }
        }
        PlaybackBackend::Auto => {
            let mut forced = Vec::new();
            if cfg!(feature = "wayland") {
                forced.push("--backend wayland");
            }
            if cfg!(feature = "x11") {
                forced.push("--backend x11");
            }
            let hint = if forced.is_empty() {
                "This build has no playback backends enabled."
            } else if forced.len() == 1 {
                "Try passing the available backend flag to force it."
            } else {
                "Try forcing a backend."
            };

            Err(anyhow!(
                "No supported playback backend detected. {details} \n\
                 {hint} {}",
                forced.join(" or "),
                details = backend_unavailable_message(),
                hint = hint,
            ))
        }
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
    #[cfg(all(not(feature = "wayland"), not(feature = "x11")))]
    let _ = (plan, countdown_secs, trace, seat_name);

    let backend = resolve_backend(backend)?;

    match backend {
        PlaybackBackend::Wayland => {
            #[cfg(feature = "wayland")]
            {
                backends::wayland::play_plan_wayland(plan, countdown_secs, trace, seat_name)
            }

            #[cfg(not(feature = "wayland"))]
            {
                let _ = seat_name;
                Err(anyhow!(
                    "Wayland backend is disabled in this build (rebuild with `--features wayland`)."
                ))
            }
        }
        PlaybackBackend::X11 => {
            if seat_name.is_some() {
                return Err(anyhow!(
                    "--seat is Wayland-only and is not supported on X11"
                ));
            }

            #[cfg(feature = "x11")]
            {
                backends::x11::play_plan_x11(plan, countdown_secs, trace)
            }

            #[cfg(not(feature = "x11"))]
            {
                Err(anyhow!(
                    "X11 backend is disabled in this build (rebuild with `--features x11`)."
                ))
            }
        }
        PlaybackBackend::Auto => Err(anyhow!("no backend resolved")),
    }
}
