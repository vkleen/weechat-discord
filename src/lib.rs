extern crate libc;
extern crate serenity;
#[macro_use]
extern crate lazy_static;
extern crate parsing;

#[macro_use]
mod synchronization;
mod buffers;
mod discord;
mod ffi;
mod hook;
mod printing;
mod utils;

pub use ffi::{get_option, wdr_end, wdr_init, MAIN_BUFFER};

// Called when plugin is loaded in Weechat
pub fn init(args: &[String]) -> Option<()> {
    hook::init();
    synchronization::init();

    if let Some(autostart) = get_option("autostart") {
        if !args.contains(&"-a".to_owned()) {
            if autostart == "true" {
                if let Some(t) = ffi::get_option("token") {
                    discord::init(&t);
                }
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
