use crate::discord::event_handler::{Handler, WeecordEvent};
use serenity::{client::bridge::gateway::ShardManager, prelude::*};
use std::{
    sync::{mpsc, Arc},
    thread,
};

pub struct DiscordClient {
    shard_manager: Arc<Mutex<ShardManager>>,
}

impl DiscordClient {
    pub fn start(token: &str) -> Result<(DiscordClient, mpsc::Receiver<WeecordEvent>), ()> {
        let (tx, rx) = mpsc::channel();
        let handler = Handler(Arc::new(Mutex::new(tx)));

        let mut client = match Client::new(token, handler) {
            Ok(client) => client,
            Err(_err) => return Err(())?,
        };

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
