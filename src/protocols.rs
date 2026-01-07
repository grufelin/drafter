pub mod virtual_keyboard_unstable_v1 {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocol/virtual-keyboard-unstable-v1.xml");
    }

    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocol/virtual-keyboard-unstable-v1.xml");
}
