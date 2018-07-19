use ffi::{self, Buffer};

mod event_handler;
pub mod formatting;

use std::sync::mpsc;
use std::thread;

use serenity::builder::GetMessages;
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
        let guild_name_id = guild.id.0.to_string();
        let buffer = if let Some(buffer) = Buffer::search(&guild_name_id) {
            buffer
        } else {
            Buffer::new(&guild_name_id, |_, _| {}).unwrap()
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

            buffer.set("short_name", &channel.name);
            buffer.set("localvar_set_channelid", &name_id);
            buffer.set("localvar_set_guildid", &guild_name_id);
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
        }
    }
}

pub fn load_history(buffer: &Buffer) {
    if let Some(channel) = buffer.get("localvar_channelid") {
        if let Some(_) = buffer.get("localvar_loaded_history") {
            return;
        }
        buffer.set("localvar_set_loaded_history", "true");
        let channel = match channel.parse::<u64>() {
            Ok(v) => ChannelId(v),
            Err(_) => return,
        };

        let retriever = GetMessages::default().limit(25);

        if let Ok(msgs) = channel.messages(|_| retriever) {
            for msg in msgs.iter().rev().cloned() {
                formatting::display_msg(&buffer, &msg, false);
            }
        }
    }
}

pub fn load_nicks(buffer: &Buffer) {
    if let Some(guild_id) = buffer.get("localvar_guildid") {
        if let Some(channel_id) = buffer.get("localvar_channelid") {
            if let Some(_) = buffer.get("localvar_loaded_nicks") {
                return;
            }
            buffer.set("localvar_set_loaded_nicks", "true");
            buffer.set("nicklist", "1");

            let guild_id = match guild_id.parse::<u64>() {
                Ok(v) => GuildId(v),
                Err(_) => return,
            };

            let channel_id = match channel_id.parse::<u64>() {
                Ok(v) => ChannelId(v),
                Err(_) => return,
            };

            let guild = guild_id.find().expect("No cache item");

            let guild_lock = guild.read();

            let members = &guild_lock.members;
            for (user_id, member) in members {
                let member_perms = guild_lock.permissions_in(channel_id, user_id);
                if !member_perms.send_messages()
                    || !member_perms.read_message_history()
                    || !member_perms.read_messages()
                {
                    continue;
                } else {
                    buffer.add_nick(member.display_name().as_ref());
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
