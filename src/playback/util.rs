use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(crate) fn sleep_interruptible(stop: &AtomicBool, ms: u64) {
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

pub(crate) fn print_trace_line(line: &str) {
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
