extern crate libc;
extern crate serenity;
#[macro_use]
extern crate lazy_static;
extern crate parsing;

mod buffers;
mod discord;
mod ffi;
mod hook;
mod printing;

pub use ffi::{get_option, wdr_end, wdr_init, MAIN_BUFFER};

// Called when plugin is loaded in Weechat
pub fn init() -> Option<()> {
    hook::init();

    if let Some(autostart) = get_option("autostart") {
        if autostart == "true" {
            if let Some(t) = ffi::get_option("token") {
                discord::init(&t);
            }
        }
    }
    Some(())
}

// Called when plugin is unloaded from Weechat
pub fn end() -> Option<()> {
    hook::destroy();
    Some(())
}

pub fn plugin_print(message: &str) {
    MAIN_BUFFER.print(&format!("weecord: {}", message));
}
