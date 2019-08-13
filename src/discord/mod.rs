use self::client::DiscordClient;
use lazy_static::lazy_static;
use serenity::{client::Context, prelude::Mutex};
use std::{sync::Arc, thread};
use weechat::Weechat;

mod client;
mod event_handler;
pub mod formatting;

pub use event_handler::TYPING_EVENTS;

pub static mut CONTEXT: Option<Context> = None;

pub fn get_ctx() -> Option<&'static Context> {
    unsafe { CONTEXT.as_ref() }
}

lazy_static! {
    pub(crate) static ref DISCORD: Arc<Mutex<Option<DiscordClient>>> = Arc::new(Mutex::new(None));
}

pub fn init(weechat: &Weechat, token: &str, irc_mode: bool) {
    let (discord_client, events) = match DiscordClient::start(weechat, token) {
        Ok(d) => d,
        Err(e) => {
            weechat.print(&format!(
                "discord: An error occurred connecting to discord: {}",
                e
            ));
            return;
        }
    };

    thread::spawn(move || {
        if let Ok(ready) = events.recv() {
            crate::plugin_print("Discord connected");
            if irc_mode {
                crate::buffers::create_autojoin_buffers(&ready);
            } else {
                crate::buffers::create_buffers(&ready);
            }
        }
    });

    *DISCORD.lock() = Some(discord_client);
}
