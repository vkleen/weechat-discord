#[macro_use]
mod synchronization;
mod buffers;
mod discord;
mod ffi;
mod hook;
mod printing;
mod utils;

use crate::ffi::get_option;
pub use crate::ffi::{wdr_end, wdr_init, MAIN_BUFFER};

// Called when plugin is loaded in Weechat
pub fn init(args: &[&str]) -> Option<()> {
    hook::init();

    if let Some(autostart) = get_option("autostart") {
        if !args.contains(&"-a") {
            if autostart == "true" {
                if let Some(t) = ffi::get_option("token") {
                    discord::init(&t, utils::get_irc_mode());
                } else {
                    plugin_print("Error: plugins.var.weecord.token unset. Run:");
                    plugin_print("/discord token 123456789ABCDEF");
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
