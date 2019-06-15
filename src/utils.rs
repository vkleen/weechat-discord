use crate::ffi::{get_option, Buffer};
use serenity::{
    cache::CacheRwLock,
    model::id::{ChannelId, GuildId},
    model::prelude::*,
    prelude::*,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum GuildOrChannel {
    Guild(GuildId),
    Channel(Option<GuildId>, ChannelId),
}

impl PartialEq<GuildId> for GuildOrChannel {
    fn eq(&self, other: &GuildId) -> bool {
        match self {
            GuildOrChannel::Guild(this_id) => this_id == other,
            GuildOrChannel::Channel(_, _) => false,
        }
    }
}

impl PartialEq<ChannelId> for GuildOrChannel {
    fn eq(&self, other: &ChannelId) -> bool {
        match self {
            GuildOrChannel::Guild(_) => false,
            GuildOrChannel::Channel(_, this_id) => this_id == other,
        }
    }
}

pub fn buffer_id_for_guild(id: GuildId) -> String {
    format!("{}", id.0)
}

pub fn buffer_id_for_channel(guild_id: Option<GuildId>, channel_id: ChannelId) -> String {
    if let Some(guild_id) = guild_id {
        format!("{}.{}", guild_id, channel_id.0)
    } else {
        format!("Private.{}", channel_id.0)
    }
}

pub fn buffer_is_muted(buffer: &Buffer) -> bool {
    if let Some(muted) = buffer.get("localvar_muted") {
        &muted == "1"
    } else {
        false
    }
}

pub fn rgb_to_ansi(color: serenity::utils::Colour) -> u8 {
    let r = (u16::from(color.r()) * 5 / 255) as u8;
    let g = (u16::from(color.g()) * 5 / 255) as u8;
    let b = (u16::from(color.b()) * 5 / 255) as u8;
    16 + 36 * r + 6 * g + b
}

pub fn status_is_online(status: OnlineStatus) -> bool {
    use OnlineStatus::*;
    match status {
        Online | Idle | DoNotDisturb => true,
        Offline | Invisible => false,
        _ => unreachable!(),
    }
}

pub fn channel_name(channel: &Channel) -> String {
    use std::borrow::Cow;
    use Channel::*;
    match channel {
        Guild(channel) => channel.read().name().to_string(),
        Group(channel) => match channel.read().name() {
            Cow::Borrowed(name) => name.to_string(),
            Cow::Owned(name) => name,
        },
        Category(category) => category.read().name().to_string(),
        Private(channel) => channel.read().name(),
        __Nonexhaustive => unreachable!(),
    }
}

/// Find the highest hoisted role (used for the user group) and the highest role (used for user coloring)
pub fn find_highest_roles(cache: &CacheRwLock, member: &Member) -> Option<(Role, Role)> {
    let mut roles = member.roles(cache)?;
    roles.sort();
    let highest = roles.last();

    let highest_hoisted = roles.iter().filter(|role| role.hoist).collect::<Vec<_>>();
    let highest_hoisted = highest_hoisted.last().cloned();
    Some((highest_hoisted?.clone(), highest?.clone()))
}

pub fn get_irc_mode() -> bool {
    get_option("irc_mode").map(|x| x == "true").unwrap_or(false)
}

pub fn unique_id(guild: Option<GuildId>, channel: ChannelId) -> String {
    if let Some(guild) = guild {
        format!("G{:?}C{}", guild.0, channel.0)
    } else {
        format!("C{}", channel.0)
    }
}

pub fn unique_guild_id(guild: GuildId) -> String {
    format!("G{}", guild)
}

pub fn parse_id(id: &str) -> Option<GuildOrChannel> {
    // id has channel part
    if let Some(c_start) = id.find('C') {
        let guild_id = id[1..c_start].parse().ok()?;
        let channel_id = id[c_start + 1..].parse().ok()?;

        Some(GuildOrChannel::Channel(Some(GuildId(guild_id)), channel_id))
    } else {
        // id is only a guild
        let guild_id = id[1..].parse().ok()?;
        Some(GuildOrChannel::Guild(GuildId(guild_id)))
    }
}

pub fn search_channel(
    cache: &CacheRwLock,
    guild_name: &str,
    channel_name: &str,
) -> Option<(Arc<RwLock<Guild>>, Arc<RwLock<GuildChannel>>)> {
    if let Some(raw_guild) = search_guild(cache, guild_name) {
        let guild = raw_guild.read();
        for channel in guild.channels.values() {
            let channel_lock = channel.read();
            if parsing::weechat_arg_strip(&channel_lock.name).to_lowercase()
                == channel_name.to_lowercase()
            {
                // Skip non text channels
                use serenity::model::channel::ChannelType::*;
                match channel_lock.kind {
                    Text | Private | Group | News => {}
                    _ => continue,
                }
                return Some((raw_guild.clone(), channel.clone()));
            }
        }
    }
    None
}

pub fn search_guild(cache: &CacheRwLock, guild_name: &str) -> Option<Arc<RwLock<Guild>>> {
    for guild in cache.read().guilds.values() {
        if parsing::weechat_arg_strip(&guild.read().name).to_lowercase()
            == guild_name.to_lowercase()
        {
            return Some(guild.clone());
        }
    }
    None
}
