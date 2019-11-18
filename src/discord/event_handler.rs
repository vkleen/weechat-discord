use crate::{buffers, on_main, on_main_blocking, printing, utils, Discord};
use lazy_static::lazy_static;
use serenity::{model::gateway::Ready, model::prelude::*, prelude::*};
use std::sync::{mpsc::Sender, Arc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use weechat::hdata::{HData, HDataPointer};
use weechat::{Buffer, ConfigOption, HasHData, Weechat};

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
    pub fn new(weecord: &Discord, sender: Arc<Mutex<Sender<Ready>>>) -> Handler {
        let watched_channels = weecord.config.watched_channels();
        let typing_messages = weecord.config.typing_messages.value();

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
                                &ctx.cache,
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
                                &ctx.cache,
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

    fn message_update(
        &self,
        ctx: Context,
        _old_if_available: Option<Message>,
        _new: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        if let Some(channel) = ctx.cache.read().channels.get(&event.channel_id) {
            let guild_id = channel.read().guild_id;
            let buffer_name = utils::buffer_id_for_channel(Some(guild_id), event.channel_id);

            thread::spawn(move || {
                let ctx = match crate::discord::get_ctx() {
                    Some(ctx) => ctx,
                    _ => return,
                };

                let mut msgs = match event
                    .channel_id
                    .messages(ctx, |retriever| retriever.limit(1).around(event.id))
                {
                    Ok(msg) => msg,
                    _ => {
                        return;
                    }
                };
                let msg = match msgs.pop() {
                    Some(msg) => msg,
                    None => return,
                };

                on_main(move |weecord| {
                    let ctx = match crate::discord::get_ctx() {
                        Some(ctx) => ctx,
                        _ => return,
                    };

                    let (_, new_content) =
                        crate::printing::render_msg(&ctx.cache, weecord, &msg, Some(guild_id));

                    modify_buffer_lines(weecord, msg.id, buffer_name, new_content);
                })
            });
        }
    }

    fn ready(&self, ctx: Context, ready: Ready) {
        // Cache seems not to have all fields properly populated

        ctx.shard
            .chunk_guilds(ready.guilds.iter().map(|g| g.id()), None, None);
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

                    // TODO: Why are channels not populated by serenity?
                    for (id, chan) in guild.channels.clone() {
                        ctx_lock.channels.insert(id, chan);
                    }
                }
            }
        }
        if let Some(presence) = ctx.cache.read().presences.get(&ready.user.id) {
            *crate::command::LAST_STATUS.lock() = presence.status;
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
    printing::inject_msg_id(msg.id, buffer);
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

fn modify_buffer_lines(
    weecord: &Discord,
    message_id: MessageId,
    buffer_name: String,
    new_content: String,
) {
    let buffer = match weecord.buffer_search("weecord", &buffer_name) {
        Some(buf) => buf,
        None => return,
    };
    if buffer.get_localvar("loaded_history").is_none() {
        return;
    }

    let buffer_hdata = buffer.get_hdata("buffer").unwrap();
    let lines_ptr: HDataPointer = buffer_hdata.get_var("own_lines").unwrap();
    let lines_hdata = lines_ptr.get_hdata("lines").unwrap();
    let mut maybe_last_line_ptr = lines_hdata.get_var::<HDataPointer>("last_line");

    let mut pointers = Vec::new();

    fn get_msg_id(last_line_hdata: &HData) -> u64 {
        let line_data_ptr: HDataPointer = last_line_hdata.get_var("data").unwrap();

        let line_data_hdata = line_data_ptr.get_hdata("line_data").unwrap();

        unsafe { line_data_hdata.get_i64_unchecked("date_printed") as u64 }
    }

    // advance to the edited message
    while let Some(last_line_ptr) = &maybe_last_line_ptr {
        let last_line_hdata = last_line_ptr.get_hdata("line").unwrap();

        if get_msg_id(&last_line_hdata) == message_id.0 {
            break;
        }

        maybe_last_line_ptr = last_line_ptr.advance(&last_line_hdata, -1);
    }

    // collect all lines of the message
    while let Some(last_line_ptr) = maybe_last_line_ptr {
        let last_line_hdata = last_line_ptr.get_hdata("line").unwrap();

        if get_msg_id(&last_line_hdata) != message_id.0 {
            break;
        }

        let line_data_ptr: HDataPointer = last_line_hdata.get_var("data").unwrap();

        let line_data_hdata = line_data_ptr.get_hdata("line_data").unwrap();

        pointers.push(line_data_hdata);

        maybe_last_line_ptr = last_line_ptr.advance(&last_line_hdata, -1);
    }

    let new_lines = new_content.splitn(pointers.len(), "\n");
    let new_lines = new_lines.map(|l| l.replace("\n", " | "));
    let new_lines = new_lines.chain(std::iter::repeat("".to_owned()));

    for (line_ptr, new_line) in pointers.iter().rev().zip(new_lines) {
        line_ptr.update_var("message", new_line);
    }
}
