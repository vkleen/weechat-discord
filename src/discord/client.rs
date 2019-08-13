use super::event_handler::Handler;
use serenity::{client::bridge::gateway::ShardManager, model::gateway::Ready, prelude::*};
use std::{
    sync::{mpsc, Arc},
    thread,
};
use weechat::Weechat;

pub struct DiscordClient {
    shard_manager: Arc<Mutex<ShardManager>>,
}

impl DiscordClient {
    pub fn start(
        weechat: &Weechat,
        token: &str,
    ) -> Result<(DiscordClient, mpsc::Receiver<Ready>), serenity::Error> {
        let (tx, rx) = mpsc::channel();
        let handler = Handler::new(weechat, Arc::new(Mutex::new(tx)));

        let mut client = Client::new(token, handler)?;

        let shard_manager = client.shard_manager.clone();
        thread::spawn(move || {
            client.start_shards(1).unwrap();
        });
        Ok((DiscordClient { shard_manager }, rx))
    }

    pub fn shutdown(&self) {
        self.shard_manager.lock().shutdown_all();
    }
}
