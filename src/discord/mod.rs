mod discord_client;
mod event_handler;
pub mod format;

use self::discord_client::DiscordClient;
use crate::MAIN_BUFFER;
use lazy_static::lazy_static;
use serenity::prelude::Mutex;
use std::{sync::Arc, thread};

lazy_static! {
    pub(crate) static ref DISCORD: Arc<Mutex<Option<DiscordClient>>> = Arc::new(Mutex::new(None));
}

pub fn init(token: &str) {
    MAIN_BUFFER.print("Connecting to Discord...");
    let (discord_client, events) = DiscordClient::start(token).unwrap();

    thread::spawn(move || {
        if let Ok(event_handler::WeecordEvent::Ready(ready)) = events.recv() {
            MAIN_BUFFER.print("Connected to Discord!");

            crate::buffers::create_buffers(&ready);
        }
    });

    *DISCORD.lock() = Some(discord_client);
}
