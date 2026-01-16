use drafter::playback::{resolve_backend, PlaybackBackend};

fn unset(name: &str) {
    std::env::remove_var(name);
}

fn set(name: &str, value: &str) {
    std::env::set_var(name, value);
}

#[test]
fn auto_prefers_wayland_when_both_present() {
    unset("WAYLAND_SOCKET");
    set("WAYLAND_DISPLAY", "wayland-1");
    set("DISPLAY", ":0");

    let resolved = resolve_backend(PlaybackBackend::Auto).expect("should resolve");
    assert_eq!(resolved, PlaybackBackend::Wayland);
}

#[test]
fn auto_errors_on_x11_only() {
    unset("WAYLAND_DISPLAY");
    unset("WAYLAND_SOCKET");
    set("DISPLAY", ":0");

    let err = resolve_backend(PlaybackBackend::Auto).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("X11"), "expected mention of X11, got: {msg}");
    assert!(
        msg.contains("not supported"),
        "expected 'not supported' wording, got: {msg}"
    );
}

#[test]
fn explicit_x11_is_rejected() {
    unset("WAYLAND_DISPLAY");
    unset("WAYLAND_SOCKET");
    unset("DISPLAY");

    let err = resolve_backend(PlaybackBackend::X11).unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("X11"));
    assert!(msg.contains("not supported"));
}
