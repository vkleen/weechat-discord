use crate::{buffers, ffi::Buffer, printing, utils};
use serenity::client::bridge::gateway::Message as WsMessage;
use serenity::{model::prelude::*, prelude::*};
use std::sync::{mpsc::Sender, Arc};

pub enum WeecordEvent {
    Ready(::serenity::model::gateway::Ready),
}

pub struct Handler {
    sender: Arc<Mutex<Sender<WeecordEvent>>>,
    watched_channels: Vec<utils::GuildOrChannel>,
}

impl Handler {
    pub fn new(sender: Arc<Mutex<Sender<WeecordEvent>>>) -> Handler {
        let watched_channels =
            crate::ffi::get_option("watched_channels").unwrap_or_else(|| "".to_string());

        let watched_channels = watched_channels
            .split(',')
            .filter(|i| !i.is_empty())
            .filter_map(utils::parse_id)
            .collect();

        Handler {
            sender,
            watched_channels,
        }
    }
}

impl EventHandler for Handler {
    fn channel_create(&self, _ctx: Context, channel: Arc<RwLock<GuildChannel>>) {
        let channel = channel.read();
        print_guild_status_message(
            channel.guild_id,
            &format!(
                "New {} channel {} created",
                channel.kind.name(),
                channel.name()
            ),
        );
    }

    fn channel_delete(&self, _ctx: Context, channel: Arc<RwLock<GuildChannel>>) {
        let channel = channel.read();
        print_guild_status_message(
            channel.guild_id,
            &format!("Channel {} deleted", channel.name()),
        );
    }

    // Called when a message is received
    fn message(&self, ctx: Context, msg: Message) {
        let string_channel = utils::buffer_id_for_channel(msg.guild_id, msg.channel_id);
        on_main! {{
            if let Some(buffer) = Buffer::search(&string_channel) {
                let muted = utils::buffer_is_muted(&buffer);
                let notify = !msg.is_own(ctx.cache) && !muted;
                printing::print_msg(&buffer, &msg, notify);
            } else {
                match msg.channel_id.to_channel(&ctx) {
                    chan @ Ok(Channel::Private(_)) => {
                        if let Some(buffer) = Buffer::search(&string_channel) {
                            let muted = utils::buffer_is_muted(&buffer);
                            let notify = !msg.is_own(ctx.cache) && !muted;
                            printing::print_msg(&buffer, &msg, notify);
                        } else {
                            // TODO: Implement "switch_to"
                            buffers::create_buffer_from_dm(
                                chan.unwrap(),
                                &ctx.cache.read().user.name,
                                false,
                            );
                        }
                    }
                    chan @ Ok(Channel::Group(_)) => {
                        if let Some(buffer) = Buffer::search(&string_channel) {
                            let muted = utils::buffer_is_muted(&buffer);
                            let notify = !msg.is_own(ctx.cache) && !muted;
                            printing::print_msg(&buffer, &msg, notify);
                        } else {
                            buffers::create_buffer_from_group(
                                chan.unwrap(),
                                &ctx.cache.read().user.name,
                            );
                        }
                    }
                    Ok(Channel::Guild(channel)) => {
                        // Check that the channel is on the watch list
                        let channel = channel.read();

                        for watched in &self.watched_channels {
                            use utils::GuildOrChannel::*;
                            let add = match watched {
                                Channel(_, channel_id) => *channel_id == channel.id,
                                Guild(guild_id) => *guild_id == channel.guild_id,
                            };
                            if add {
                                let guild = match channel.guild_id.to_guild_cached(&ctx.cache) {
                                    Some(guild) => guild,
                                    None => return,
                                };

                                let current_user = ctx.cache.read().user.clone();
                                let guild = guild.read();

                                // TODO: Colors?
                                let nick = if let Ok(current_member) =
                                    guild.id.member(&ctx, current_user.id)
                                {
                                    format!("@{}", current_member.display_name())
                                } else {
                                    format!("@{}", current_user.name)
                                };

                                buffers::create_guild_buffer(guild.id, &guild.name);
                                // TODO: Muting
                                buffers::create_buffer_from_channel(&ctx.cache, &channel, &nick, false);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }};
    }

    fn ready(&self, ctx: Context, ready: Ready) {
        // Opcode 12 is undocumented "guild sync"
        let data = serde_json::json!({
            "op": 12,
            "d": ready.guilds.iter().map(|g| g.id().0.to_string()).collect::<Vec<_>>()
        });
        ctx.shard
            .websocket_message(WsMessage::text(data.to_string()));
        // Cache seems not to have private channels properly populated
        {
            let mut ctx_lock = ctx.cache.write();
            for (&id, channel) in &ready.private_channels {
                if let Some(pc) = channel.clone().private() {
                    ctx_lock.private_channels.insert(id, pc);
                }
            }
        }
        unsafe {
            crate::discord::CONTEXT = Some(ctx);
        }
        let _ = self.sender.lock().send(WeecordEvent::Ready(ready));
    }

    fn typing_start(&self, ctx: Context, event: TypingStartEvent) {
        if event.user_id == ctx.cache.read().user.id {
            return;
        }
        let buffer_id = crate::utils::buffer_id_for_channel(event.guild_id, event.channel_id);
        if let Some(buffer) = crate::ffi::Buffer::search(&buffer_id) {
            let prefix = crate::ffi::get_prefix("network").unwrap_or_else(|| "".to_string());
            let user = event
                .user_id
                .to_user_cached(ctx.cache)
                .map(|user| user.read().name.clone())
                .unwrap_or_else(|| "Someone".to_string());
            buffer.print(&format!("{}\t{} is typing", prefix, user));
        }
    }
}

fn print_guild_status_message(guild_id: GuildId, msg: &str) {
    let buffer_id = utils::buffer_id_for_guild(guild_id);

    if let Some(buffer) = Buffer::search(&buffer_id) {
        let prefix = crate::ffi::get_prefix("network").unwrap_or_else(|| " ".to_string());
        buffer.print(&(prefix + "\t" + msg));
    }
}
