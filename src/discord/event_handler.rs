use crate::{buffers, on_main, on_main_blocking, printing, utils};
use lazy_static::lazy_static;
use serenity::{
    client::bridge::gateway::Message as WsMessage, model::gateway::Ready, model::prelude::*,
    prelude::*,
};
use std::sync::{mpsc::Sender, Arc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use weechat::{Buffer, Weechat};

const MAX_TYPING_EVENTS: usize = 50;

#[derive(Debug, PartialEq, Eq, Ord)]
pub struct TypingEntry {
    pub channel_id: ChannelId,
    pub guild_id: Option<GuildId>,
    pub user: UserId,
    pub user_name: String,
    pub time: u64,
}

impl PartialOrd for TypingEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.time.partial_cmp(&other.time)
    }
}

pub struct TypingTracker {
    pub entries: Vec<TypingEntry>,
}

impl TypingTracker {
    /// Remove any expired entries
    pub fn sweep(&mut self) {
        let now = SystemTime::now();
        let timestamp_now = now
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as u64;

        // If the entry is more than 10 seconds old, remove it
        // TODO: Use binary heap or other structure for better performance?
        self.entries.retain(|e| timestamp_now - e.time < 10)
    }
}

lazy_static! {
    pub static ref TYPING_EVENTS: Arc<Mutex<TypingTracker>> = Arc::new(Mutex::new(TypingTracker {
        entries: Vec::new(),
    }));
}

pub struct Handler {
    sender: Arc<Mutex<Sender<Ready>>>,
    watched_channels: Vec<utils::GuildOrChannel>,
    typing_messages: bool,
}

impl Handler {
    pub fn new(weechat: &Weechat, sender: Arc<Mutex<Sender<Ready>>>) -> Handler {
        let watched_channels = weechat.get_plugin_option("watched_channels").unwrap_or("");

        let typing_messages = weechat
            .get_plugin_option("typing_messages")
            .map(|v| v == "true")
            .unwrap_or(false);

        let watched_channels = watched_channels
            .split(',')
            .filter(|i| !i.is_empty())
            .filter_map(utils::parse_id)
            .collect();

        Handler {
            sender,
            watched_channels,
            typing_messages,
        }
    }
}

impl EventHandler for Handler {
    fn channel_create(&self, _ctx: Context, channel: Arc<RwLock<GuildChannel>>) {
        let channel = channel.read();
        print_guild_status_message(
            channel.guild_id,
            &format!(
                "New {} channel `{}` created",
                channel.kind.name(),
                channel.name()
            ),
        );
    }

    fn channel_delete(&self, _ctx: Context, channel: Arc<RwLock<GuildChannel>>) {
        let channel = channel.read();
        print_guild_status_message(
            channel.guild_id,
            &format!("Channel `{}` deleted", channel.name()),
        );
    }

    fn channel_update(&self, ctx: Context, old: Option<Channel>, new: Channel) {
        // TODO: Notify more events?
        // * Groups: user learve/join
        // * guild channel: ?
        match new {
            Channel::Category(new) => {
                // TODO: old doesn't ever seem to be available
                if let Some(old) = old.and_then(|old| old.category()) {
                    let new = new.read();
                    let old = old.read();

                    let guild_id = new
                        .id
                        .to_channel_cached(&ctx)
                        .and_then(|ch| ch.guild())
                        .map(|ch| ch.read().guild_id);

                    if let Some(guild_id) = guild_id {
                        if new.name != old.name {
                            print_guild_status_message(
                                guild_id,
                                &format!("Category `{}` renamed to `{}`", old.name, new.name),
                            );
                        }
                    }
                }
            }
            Channel::Guild(new) => {
                if let Some(old) = old.and_then(|old| old.guild()) {
                    let new = new.read();
                    let old = old.read();

                    if new.name != old.name {
                        print_guild_status_message(
                            new.guild_id,
                            &format!("Category `{}` renamed to `{}`", old.name, new.name),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn guild_member_update(&self, ctx: Context, old: Option<Member>, new: Member) {
        thread::spawn(move || {
            buffers::update_member_nick(&old, &new);
            if ctx.cache.read().user.id == new.user_id() {
                buffers::update_nick();
            }
        });
    }

    fn message(&self, ctx: Context, msg: Message) {
        let string_channel = utils::buffer_id_for_channel(msg.guild_id, msg.channel_id);
        let () = on_main_blocking(move |weechat| {
            if let Some(buffer) = weechat.buffer_search("weecord", &string_channel) {
                print_message(&weechat, ctx, &msg, &buffer);
            } else {
                match msg.channel_id.to_channel(&ctx) {
                    chan @ Ok(Channel::Private(_)) => {
                        if let Some(buffer) = weechat.buffer_search("weecord", &string_channel) {
                            print_message(&weechat, ctx, &msg, &buffer);
                        } else {
                            buffers::create_buffer_from_dm(
                                &weechat,
                                chan.unwrap(),
                                &ctx.cache.read().user.name,
                                false,
                            );
                        }
                    }
                    chan @ Ok(Channel::Group(_)) => {
                        if let Some(buffer) = weechat.buffer_search("weecord", &string_channel) {
                            print_message(&weechat, ctx, &msg, &buffer);
                        } else {
                            buffers::create_buffer_from_group(
                                &weechat,
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
                                buffers::create_buffer_from_channel(
                                    &ctx.cache,
                                    &guild.name,
                                    &channel,
                                    &nick,
                                    false,
                                );
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    fn ready(&self, ctx: Context, ready: Ready) {
        // Opcode 12 is undocumented "guild sync" which forces all guilds to be sent to the client
        let data = object! {
            "op" => 12,
            "d" => ready.guilds.iter().map(|g| g.id().0.to_string()).collect::<Vec<_>>()
        };
        ctx.shard
            .websocket_message(WsMessage::text(data.to_string()));
        // Cache seems not to have all fields properly populated
        {
            let mut ctx_lock = ctx.cache.write();
            for (&id, channel) in &ready.private_channels {
                if let Some(pc) = channel.clone().private() {
                    ctx_lock.private_channels.insert(id, pc);
                }
            }
            for guild in ready.guilds.iter() {
                if let GuildStatus::OnlineGuild(guild) = guild {
                    for (id, pres) in guild.presences.clone() {
                        ctx_lock.presences.insert(id, pres);
                    }
                }
            }
        }
        unsafe {
            crate::discord::CONTEXT = Some(ctx);
        }
        let _ = self.sender.lock().send(ready);
    }

    fn typing_start(&self, ctx: Context, event: TypingStartEvent) {
        // TODO: Do we want to fetch the user if it isn't cached? (check performance)
        if let Some(user) = event.user_id.to_user_cached(&ctx.cache) {
            // TODO: Resolve guild nick names
            let mut typing_events = TYPING_EVENTS.lock();
            typing_events.entries.push(TypingEntry {
                channel_id: event.channel_id,
                guild_id: event.guild_id,
                user: event.user_id,
                user_name: user.read().name.clone(),
                time: event.timestamp,
            });

            typing_events.sweep();
            if typing_events.entries.len() > MAX_TYPING_EVENTS {
                typing_events.entries.pop();
            }

            crate::on_main(|weechat| {
                weechat.update_bar_item("discord_typing");
            });

            thread::Builder::new()
                .name("Typing indicator updater".into())
                .spawn(|| {
                    // Wait a few seconds, then sweep the list and update the bar item
                    thread::sleep(Duration::from_secs(10));

                    let mut typing_events = TYPING_EVENTS.lock();
                    typing_events.sweep();
                    crate::on_main(|weechat| {
                        weechat.update_bar_item("discord_typing");
                    });
                })
                .expect("Unable to name thread");
        }

        if self.typing_messages {
            if event.user_id == ctx.cache.read().user.id {
                return;
            }
            let buffer_id = crate::utils::buffer_id_for_channel(event.guild_id, event.channel_id);
            on_main(move |weechat| {
                if let Some(buffer) = weechat.buffer_search("weecord", &buffer_id) {
                    let prefix = weechat.get_prefix("network");
                    let user = event
                        .user_id
                        .to_user_cached(ctx.cache)
                        .map(|user| user.read().name.clone())
                        .unwrap_or_else(|| "Someone".to_string());
                    buffer.print(&format!("{}\t{} is typing", prefix, user));
                }
            })
        }
    }

    fn user_update(&self, _ctx: Context, _old: CurrentUser, _new: CurrentUser) {
        thread::spawn(|| {
            // TODO: Update nicklist (and/or just rework all nick stuff)
            buffers::update_nick();
        });
    }
}

fn print_message(weechat: &Weechat, ctx: Context, msg: &Message, buffer: &Buffer) {
    let muted = utils::buffer_is_muted(&buffer);
    let notify = !msg.is_own(ctx.cache) && !muted;
    printing::print_msg(&weechat, &buffer, &msg, notify);
}

fn print_guild_status_message(guild_id: GuildId, msg: &str) {
    let buffer_id = utils::buffer_id_for_guild(guild_id);

    let msg = msg.to_owned();
    on_main(move |weechat| {
        if let Some(buffer) = weechat.buffer_search("weecord", &buffer_id) {
            let prefix = weechat.get_prefix("network").to_owned();
            buffer.print(&(prefix + "\t" + msg.as_ref()));
        }
    })
}
