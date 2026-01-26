use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

use drafter::playback::{resolve_backend, PlaybackBackend};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvRestore {
    wayland_display: Option<OsString>,
    wayland_socket: Option<OsString>,
    display: Option<OsString>,
}

impl EnvRestore {
    fn snapshot() -> Self {
        Self {
            wayland_display: std::env::var_os("WAYLAND_DISPLAY"),
            wayland_socket: std::env::var_os("WAYLAND_SOCKET"),
            display: std::env::var_os("DISPLAY"),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        // SAFETY: modifying the process environment is not thread-safe in general.
        // These tests serialize all env var mutations via the `env_lock()` mutex.
        match &self.wayland_display {
            Some(v) => unsafe { std::env::set_var("WAYLAND_DISPLAY", v) },
            None => unsafe { std::env::remove_var("WAYLAND_DISPLAY") },
        }
        match &self.wayland_socket {
            Some(v) => unsafe { std::env::set_var("WAYLAND_SOCKET", v) },
            None => unsafe { std::env::remove_var("WAYLAND_SOCKET") },
        }
        match &self.display {
            Some(v) => unsafe { std::env::set_var("DISPLAY", v) },
            None => unsafe { std::env::remove_var("DISPLAY") },
        }
    }
}

fn unset(name: &str) {
    // SAFETY: callers hold the global test mutex from `env_lock()`.
    unsafe { std::env::remove_var(name) };
}

fn set(name: &str, value: &str) {
    // SAFETY: callers hold the global test mutex from `env_lock()`.
    unsafe { std::env::set_var(name, value) };
}

#[test]
fn auto_prefers_wayland_when_both_present() {
    let _guard = env_lock().lock().unwrap();
    let _restore = EnvRestore::snapshot();

    unset("WAYLAND_SOCKET");
    set("WAYLAND_DISPLAY", "wayland-1");
    set("DISPLAY", ":0");

    #[cfg(feature = "wayland")]
    {
        let resolved = resolve_backend(PlaybackBackend::Auto).expect("should resolve");
        assert_eq!(resolved, PlaybackBackend::Wayland);
    }

    #[cfg(all(not(feature = "wayland"), feature = "x11"))]
    {
        // If Wayland support is compiled out, auto should fall back to X11.
        let resolved = resolve_backend(PlaybackBackend::Auto).expect("should resolve");
        assert_eq!(resolved, PlaybackBackend::X11);
    }

    #[cfg(all(not(feature = "wayland"), not(feature = "x11")))]
    {
        let err = resolve_backend(PlaybackBackend::Auto).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("No supported playback backend"));
    }
}

#[test]
fn auto_errors_or_resolves_on_x11_only() {
    let _guard = env_lock().lock().unwrap();
    let _restore = EnvRestore::snapshot();

    unset("WAYLAND_DISPLAY");
    unset("WAYLAND_SOCKET");
    set("DISPLAY", ":0");

    #[cfg(feature = "x11")]
    {
        let resolved = resolve_backend(PlaybackBackend::Auto).expect("should resolve");
        assert_eq!(resolved, PlaybackBackend::X11);
    }

    #[cfg(not(feature = "x11"))]
    {
        let err = resolve_backend(PlaybackBackend::Auto).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("No supported playback backend detected"),
            "expected missing-backend wording, got: {msg}"
        );
        assert!(
            msg.contains("DISPLAY is set"),
            "expected mention of DISPLAY, got: {msg}"
        );
    }
}

#[test]
fn explicit_x11_is_rejected_or_accepted() {
    let _guard = env_lock().lock().unwrap();
    let _restore = EnvRestore::snapshot();

    unset("WAYLAND_DISPLAY");
    unset("WAYLAND_SOCKET");
    unset("DISPLAY");

    #[cfg(feature = "x11")]
    {
        let resolved = resolve_backend(PlaybackBackend::X11).expect("should resolve");
        assert_eq!(resolved, PlaybackBackend::X11);
    }

    #[cfg(not(feature = "x11"))]
    {
        let err = resolve_backend(PlaybackBackend::X11).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("X11"));
        assert!(msg.contains("disabled"));
    }
}
