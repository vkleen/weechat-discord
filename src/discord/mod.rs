mod discord_client;
mod event_handler;
pub mod format;

use self::discord_client::DiscordClient;
use MAIN_BUFFER;

use serenity::prelude::Mutex;
use std::sync::Arc;
use std::thread;

lazy_static! {
    pub(crate) static ref DISCORD: Arc<Mutex<Option<DiscordClient>>> = Arc::new(Mutex::new(None));
}

pub fn init(token: &str) {
    MAIN_BUFFER.print("Connecting to Discord...");
    let (discord_client, events) = DiscordClient::start(token).unwrap();

    thread::spawn(move || {
        events.recv().unwrap();
        MAIN_BUFFER.print("Connected to Discord!");

        ::buffers::create_buffers();
    });

    *DISCORD.lock() = Some(discord_client);
}
