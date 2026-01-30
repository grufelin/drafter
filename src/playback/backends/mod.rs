#[cfg(feature = "wayland")]
pub mod wayland;

#[cfg(feature = "x11")]
pub mod x11;

// Common modifiers we try to "unstick" on abort/error.
//
// Even if the planner doesn't currently use all of these, releasing them is a cheap
// defensive measure to reduce the chance of starting/ending a run with a modifier held.
// Note: if a user is physically holding a modifier while this runs, the target app's
// perceived state may temporarily desync until the key is tapped again.
pub(crate) const COMMON_MODIFIER_KEYCODES: [u32; 6] = [
    crate::keyboard::KEY_LEFTSHIFT,
    crate::keyboard::KEY_RIGHTSHIFT,
    crate::keyboard::KEY_LEFTCTRL,
    crate::keyboard::KEY_RIGHTCTRL,
    crate::keyboard::KEY_LEFTALT,
    crate::keyboard::KEY_RIGHTALT,
];

#[cfg(test)]
mod tests {
    use super::COMMON_MODIFIER_KEYCODES;

    #[test]
    fn common_modifier_list_contains_expected_keys() {
        assert!(COMMON_MODIFIER_KEYCODES.contains(&crate::keyboard::KEY_LEFTSHIFT));
        assert!(COMMON_MODIFIER_KEYCODES.contains(&crate::keyboard::KEY_RIGHTSHIFT));
        assert!(COMMON_MODIFIER_KEYCODES.contains(&crate::keyboard::KEY_LEFTCTRL));
        assert!(COMMON_MODIFIER_KEYCODES.contains(&crate::keyboard::KEY_RIGHTCTRL));
        assert!(COMMON_MODIFIER_KEYCODES.contains(&crate::keyboard::KEY_LEFTALT));
        assert!(COMMON_MODIFIER_KEYCODES.contains(&crate::keyboard::KEY_RIGHTALT));
    }
}
