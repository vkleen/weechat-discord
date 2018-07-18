use ffi::{self, Buffer};

mod event_handler;
mod formatting;

use std::sync::mpsc;
use std::thread;

use serenity::client::bridge::gateway::ShardManager;
use serenity::model::prelude::*;
use serenity::prelude::Mutex;
use serenity::{Client, CACHE};

use std::sync::Arc;

pub fn init(token: &str) -> DiscordClient {
    let discord_client = DiscordClient::start(token).unwrap();

    let current_user = CACHE.read().user.clone();

    create_buffers(&current_user);

    discord_client
}

fn create_buffers(current_user: &CurrentUser) {
    let nick = format!("@{}", current_user.name);
    for guild in current_user.guilds().unwrap() {
        let name_id = guild.id.0.to_string();
        let buffer = if let Some(buffer) = Buffer::search(&name_id) {
            buffer
        } else {
            Buffer::new(&name_id, |_, _| {}).unwrap()
        };
        buffer.set("short_name", &guild.name);
        buffer.set("localvar_set_type", "server");
        for channel in guild.id.channels().unwrap().values() {
            if let Ok(perms) = channel.permissions_for(current_user.id) {
                if !perms.send_messages() || !perms.read_message_history() {
                    continue;
                }
            }

            let channel_type = match channel.kind {
                ChannelType::Category | ChannelType::Voice => continue,
                ChannelType::Private => "private",
                ChannelType::Group | ChannelType::Text => "channel",
            };

            let name_id = channel.id.0.to_string();
            let buffer = if let Some(buffer) = Buffer::search(&name_id) {
                buffer
            } else {
                Buffer::new(&name_id, buffer_input).unwrap()
            };

            let mut short_name = channel.name.clone();
            short_name.truncate(30);
            buffer.set("short_name", &short_name);
            buffer.set("localvar_set_channelid", &name_id);
            buffer.set("localvar_set_type", channel_type);
            buffer.set("localvar_set_nick", &nick);
            let title = if let Some(ref topic) = channel.topic {
                if !topic.is_empty() {
                    format!("{} | {}", channel.name, topic)
                } else {
                    channel.name.clone()
                }
            } else {
                channel.name.clone()
            };
            buffer.set("title", &title);

            // Load history
            use serenity::builder::GetMessages;

            let retriever = GetMessages::default().limit(25);

            if let Ok(msgs) = channel.messages(|_| retriever) {
                for msg in msgs.iter().rev().cloned() {
                    // buffer.print_tags("notify_none", &msg.content);
                    formatting::display_msg(&buffer, &msg, false);
                }
            }
        }
    }
}

fn buffer_input(buffer: Buffer, message: &str) {
    let channel = buffer
        .get("localvar_channelid")
        .and_then(|id| id.parse().ok())
        .map(|id| ChannelId(id));

    let message = ffi::remove_color(message);

    if let Some(channel) = channel {
        channel
            .say(message)
            .expect(&format!("Unable to send message to {}", channel.0));
    }
}

pub struct DiscordClient {
    shard_manager: Arc<Mutex<ShardManager>>,
}

impl DiscordClient {
    pub fn start(token: &str) -> Result<DiscordClient, ()> {
        let (tx, rx) = mpsc::channel();
        let handler = event_handler::Handler(Arc::new(Mutex::new(tx)));

        let mut client = match Client::new(token, handler) {
            Ok(client) => client,
            Err(_err) => return Err(())?,
        };

        let shard_manager = client.shard_manager.clone();
        thread::spawn(move || {
            client.start_shards(1).unwrap();
        });

        rx.recv().unwrap();
        Ok(DiscordClient { shard_manager })
    }

    pub fn shutdown(&self) {
        self.shard_manager.lock().shutdown_all();
    }
}
