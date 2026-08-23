// Unix-only for the same reason as the other integration binaries; the macOS
// exclusion is separate and tracked on its own.
#![cfg(all(unix, not(target_os = "macos")))]

mod support;

#[path = "cli/mod.rs"]
mod cases;
