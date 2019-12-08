use super::event_handler::Handler;
use crate::Discord;
use serenity::{client::bridge::gateway::ShardManager, model::gateway::Ready, prelude::*};
use std::{
    sync::{mpsc, Arc},
    thread,
};

pub struct DiscordClient {
    shard_manager: Arc<Mutex<ShardManager>>,
}

impl DiscordClient {
    pub fn start(
        weecord: &Discord,
        token: &str,
    ) -> Result<(DiscordClient, mpsc::Receiver<Ready>), serenity::Error> {
        let (tx, rx) = mpsc::channel();
        let handler = Handler::new(weecord, Arc::new(Mutex::new(tx)));

        let mut client = Client::new(token, handler)?;

        let shard_manager = client.shard_manager.clone();
        thread::spawn(move || {
            if let Err(e) = client.start_shards(1) {
                crate::on_main(move |weecord| {
                    weecord.print(&format!(
                        "discord: An error occurred connecting to discord: {}",
                        e
                    ));
                });
            }
        });
        Ok((DiscordClient { shard_manager }, rx))
    }

    pub fn shutdown(&self) {
        self.shard_manager.lock().shutdown_all();
    }
}
