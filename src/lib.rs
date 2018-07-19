extern crate libc;
extern crate serenity;
#[macro_use]
extern crate lazy_static;
extern crate parsing;

mod discord;
mod ffi;

use ffi::*;

pub use ffi::wdr_end;
pub use ffi::wdr_init;

use serenity::prelude::Mutex;
use std::sync::Arc;

lazy_static! {
    static ref DISCORD: Arc<Mutex<Option<discord::DiscordClient>>> = Arc::new(Mutex::new(None));
}

mod weechat {
    pub const COMMAND: &'static str = "discord";
    pub const DESCRIPTION: &'static str = "\
Discord from the comfort of your favorite command-line IRC client!
Source code available at https://github.com/Noskcaj19/weechat-discord
Originally by https://github.com/khyperia/weechat-discord
How does channel muting work?
If plugins.var.weecord.mute.<channel_id> is set to the literal \"1\", \
then that buffer will not be opened. When a Discord channel is muted \
(in the official client), weechat-discord detects this and automatically \
sets this setting for you. If you would like to override this behavior \
and un-mute the channel, set the setting to \"0\". (Do not unset it, as it \
will just get automatically filled in again)
Options used:
plugins.var.weecord.token = <discord_token>
plugins.var.weecord.rename.<id> = <string>
plugins.var.weecord.mute.<channel_id> = (0|1)
plugins.var.weecord.on_delete.<server_id> = <channel_id>
";
    pub const ARGS: &'static str = "\
                     connect
                     disconnect
                     token <token>";
    pub const ARGDESC: &'static str = "\
connect: sign in to discord and open chat buffers
disconnect: sign out of Discord
token: set Discord login token
query: open PM buffer with user
Example:
  /discord token 123456789ABCDEF
  /discord connect
  /discord query khyperia
  /discord disconnect
";
    pub const COMPLETIONS: &'static str =
        "\
         connect || disconnect || token || autostart || noautostart || query";
}

// *DO NOT* touch this outside of init/end
static mut MAIN_COMMAND_HOOK: *mut HookCommand = 0 as *mut _;
static mut MAIN_COMMAND_HOOK2: *mut SignalHook = 0 as *mut _;

fn handle_buffer_switch(data: SignalHookData) {
    match data {
        SignalHookData::Pointer(buffer) => discord::load_history(&buffer),
        _ => {}
    }
}

// Called when plugin is loaded in Weechat
pub fn init() -> Option<()> {
    let hook = ffi::hook_command(
        weechat::COMMAND,
        weechat::DESCRIPTION,
        weechat::ARGS,
        weechat::ARGDESC,
        weechat::COMPLETIONS,
        move |buffer, input| run_command(&buffer, input),
    )?;

    ffi::hook_signal("buffer_switch", handle_buffer_switch)
        .map(|hook| unsafe { MAIN_COMMAND_HOOK2 = Box::into_raw(Box::new(hook)) });

    unsafe {
        MAIN_COMMAND_HOOK = Box::into_raw(Box::new(hook));
    };

    if let Some(autostart) = get_option("autostart") {
        if autostart == "true" {
            if let Some(t) = ffi::get_option("token") {
                *DISCORD.lock() = Some(discord::init(&t));
            }
        }
    }
    Some(())
}

// Called when plugin is unloaded from Weechat
pub fn end() -> Option<()> {
    unsafe {
        let _ = Box::from_raw(MAIN_COMMAND_HOOK);
        MAIN_COMMAND_HOOK = ::std::ptr::null_mut();
    };
    Some(())
}

fn user_set_option(name: &str, value: &str) {
    command_print(&ffi::set_option(name, value));
}

fn command_print(message: &str) {
    MAIN_BUFFER.print(&format!("{}: {}", &weechat::COMMAND, message));
}

fn run_command(_buffer: &Buffer, command: &str) {
    // TODO: Add rename command
    if command == "" {
        command_print("see /help discord for more information")
    } else if command == "connect" {
        match ffi::get_option("token") {
            Some(t) => {
                if DISCORD.lock().is_none() {
                    *DISCORD.lock() = Some(discord::init(&t))
                }
            }
            None => {
                command_print("Error: plugins.var.weecord.token unset. Run:");
                command_print("/discord token 123456789ABCDEF");
                return;
            }
        };
    } else if command == "disconnect" {
        let mut discord = DISCORD.lock();
        if discord.is_some() {
            let discord = discord.take();
            discord.unwrap().shutdown();
        }
        command_print("disconnected");
    } else if command.starts_with("token ") {
        let token = &command["token ".len()..];
        user_set_option("token", token.trim_matches('"'));
    } else if command == "autostart" {
        set_option("autostart", "true");
        command_print("Discord will now load on startup");
    } else if command == "noautostart" {
        set_option("autostart", "false");
        command_print("Discord will not load on startup");
    } else {
        command_print("unknown command");
    }
    // } else if command.starts_with("query ") {
    //     query_command(buffer, &command["query ".len()..]);
    // } else if command.starts_with("debug ") {
    //     debug_command(&command["debug ".len()..]);
    // }
}
