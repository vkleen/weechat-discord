use crate::{
    ffi::{update_bar_item, Buffer},
    printing, utils,
};
use serenity::cache::CacheRwLock;
use serenity::model::prelude::*;
use std::collections::{HashMap, VecDeque};

pub fn create_buffers(ready_data: &Ready) {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let current_user = ctx.cache.read().user.clone();

    let guilds = match current_user.guilds(&ctx.http) {
        Ok(guilds) => guilds,
        Err(e) => {
            on_main! {{
                crate::plugin_print(&format!("Error getting user guilds: {:?}", e));
            }};
            vec![]
        }
    };
    let mut map: HashMap<_, _> = guilds.iter().map(|g| (g.id, g)).collect();

    let mut sorted_guilds = VecDeque::new();

    // Add the guilds ordered from the client
    for guild_id in &ready_data.user_settings.guild_positions {
        if let Some(guild) = map.remove(&guild_id) {
            sorted_guilds.push_back(guild);
        }
    }

    // Prepend any remaning guilds
    for guild in map.values() {
        sorted_guilds.push_front(guild.clone());
    }

    for guild in &sorted_guilds {
        let guild_settings = ready_data.user_guild_settings.get(&guild.id.into());
        let guild_muted;
        let mut channel_muted = HashMap::new();
        if let Some(guild_settings) = guild_settings {
            guild_muted = guild_settings.muted;
            for (channel_id, channel_override) in guild_settings.channel_overrides.iter() {
                channel_muted.insert(channel_id, channel_override.muted);
            }
        } else {
            guild_muted = false;
        }
        create_guild_buffer(guild.id, &guild.name);

        // TODO: Colors?
        let nick = if let Ok(current_member) = guild.id.member(&ctx, current_user.id) {
            format!("@{}", current_member.display_name())
        } else {
            format!("@{}", current_user.name)
        };
        let channels = guild
            .id
            .channels(&ctx.http)
            .expect("Unable to fetch channels");
        let mut channels = channels.values().collect::<Vec<_>>();
        channels.sort_by_key(|g| g.position);
        for channel in channels {
            let is_muted =
                guild_muted || channel_muted.get(&channel.id).cloned().unwrap_or_default();
            create_buffer_from_channel(&ctx.cache, &channel, &nick, is_muted);
        }
    }
}

pub fn create_guild_buffer(id: GuildId, name: &str) {
    let guild_name_id = utils::buffer_id_for_guild(id);
    on_main! {{
        let buffer = if let Some(buffer) = Buffer::search(&guild_name_id) {
            buffer
        } else {
            Buffer::new(&guild_name_id, |_, _| {}).unwrap()
        };
        buffer.set("short_name", name);
        buffer.set("localval_set_guildid", &id.0.to_string());
        buffer.set("localvar_set_type", "server");
    }};
}

pub fn create_buffer_from_channel(
    cache: &CacheRwLock,
    channel: &GuildChannel,
    nick: &str,
    muted: bool,
) {
    let current_user = cache.read().user.clone();
    if let Ok(perms) = channel.permissions_for(cache, current_user.id) {
        if !perms.read_message_history() {
            return;
        }
    }

    let channel_type = match channel.kind {
        ChannelType::Category | ChannelType::Voice => return,
        ChannelType::Private => "private",
        ChannelType::Group | ChannelType::Text | ChannelType::News => "channel",
        _ => panic!("Unknown chanel type"),
    };

    let name_id = utils::buffer_id_for_channel(Some(channel.guild_id), channel.id);

    on_main! {{
        let buffer = if let Some(buffer) = Buffer::search(&name_id) {
            buffer
        } else {
            Buffer::new(&name_id, crate::hook::buffer_input).unwrap()
        };
        buffer.set("short_name", &channel.name);
        buffer.set("localvar_set_channelid", &channel.id.0.to_string());
        buffer.set("localvar_set_guildid", &channel.guild_id.0.to_string());
        buffer.set("localvar_set_type", channel_type);
        buffer.set("localvar_set_nick", &nick);
        let mut title = if let Some(ref topic) = channel.topic {
            if !topic.is_empty() {
                format!("{} | {}", channel.name, topic)
            } else {
                channel.name.clone()
            }
        } else {
            channel.name.clone()
        };

        if muted {
            title += " (muted)";
        }
        buffer.set("title", &title);
        buffer.set("localvar_set_muted", &(muted as u8).to_string());
    }};
}

// TODO: Reduce code duplication
/// Must be called on main
pub fn create_buffer_from_dm(channel: Channel, nick: &str, switch_to: bool) {
    let channel = match channel.private() {
        Some(chan) => chan,
        None => return,
    };
    let channel = channel.read();

    let name_id = utils::buffer_id_for_channel(None, channel.id);
    let buffer = if let Some(buffer) = Buffer::search(&name_id) {
        buffer
    } else {
        Buffer::new(&name_id, crate::hook::buffer_input).unwrap()
    };

    buffer.set("short_name", &channel.name());
    buffer.set("localvar_set_channelid", &channel.id.0.to_string());
    buffer.set("localvar_set_nick", &nick);
    if switch_to {
        buffer.set("display", "1");
    }
    let title = format!("DM with {}", channel.recipient.read().name);
    buffer.set("title", &title);
}

/// Must be called on main
pub fn create_buffer_from_group(channel: Channel, nick: &str) {
    let channel = match channel.group() {
        Some(chan) => chan,
        None => return,
    };
    let channel = channel.read();

    let title = format!(
        "DM with {}",
        channel
            .recipients
            .values()
            .map(|u| u.read().name.to_owned())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let name_id = utils::buffer_id_for_channel(None, channel.channel_id);

    let buffer = if let Some(buffer) = Buffer::search(&name_id) {
        buffer
    } else {
        Buffer::new(&name_id, crate::hook::buffer_input).unwrap()
    };

    buffer.set("short_name", &channel.name());
    buffer.set("localvar_set_channelid", &channel.channel_id.0.to_string());
    buffer.set("localvar_set_nick", &nick);
    buffer.set("title", &title);
}

// TODO: Make this nicer somehow
// TODO: Refactor this to use `?`
pub fn load_nicks(buffer: &Buffer) {
    let (guild_id, channel_id) = on_main! {{
        if buffer.get("localvar_loaded_nicks").is_some() {
            return;
        }

        let guild_id = match buffer.get("localvar_guildid") {
            Some(guild_id) => guild_id,
            None => return,
        };

        let channel_id = match buffer.get("localvar_channelid") {
            Some(channel_id) => channel_id,
            None => return,
        };

        let guild_id = match guild_id.parse::<u64>() {
            Ok(v) => GuildId(v),
            Err(_) => return,
        };

        let channel_id = match channel_id.parse::<u64>() {
            Ok(v) => ChannelId(v),
            Err(_) => return,
        };

        buffer.set("localvar_set_loaded_nicks", "true");
        buffer.set("nicklist", "1");

        (guild_id, channel_id)
    }};
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    let guild = guild_id
        .to_guild_cached(&ctx.cache)
        .expect("No guild cache item");

    let guild_lock = guild.read();

    // Typeck not smart enough
    let none_user: Option<UserId> = None;
    // TODO: What to do with more than 1000 members?
    let members = guild_lock
        .members(&ctx.http, Some(1000), none_user)
        .unwrap();
    on_main! {{
        for member in members {
            let user_id = member.user.read().id;
            let member_perms = guild_lock.permissions_in(channel_id, user_id);
            if !member_perms.send_messages()
                || !member_perms.read_message_history()
                || !member_perms.read_messages()
            {
                continue;
            } else {
                // TODO: Make hoist correctly affect position
                if let Some((role, pos)) = member.highest_role_info(&ctx.cache) {
                    if let Some(role) = role.to_role_cached(&ctx.cache) {
                        let user = member.user.read();
                        if role.hoist || user.bot {
                            let role_name;
                            let color;
                            if user.bot {
                                role_name = format!("{}|{}", ::std::i64::MAX, "Bot");
                                color = "gray".to_string();
                            } else {
                                role_name = format!("{}|{}", ::std::i64::MAX - pos, role.name);
                                color = crate::utils::rgb_to_ansi(role.colour).to_string();
                            };
                            if !buffer.group_exists(&role_name) {
                                buffer.add_nicklist_group_with_color(&role_name, &color);
                            }
                            buffer.add_nick_to_group(member.display_name().as_ref(), &role_name);
                            continue
                        }
                    }
                }
                buffer.add_nick(member.display_name().as_ref());
            }
        }
    }};
}

pub fn load_history(buffer: &Buffer) {
    let channel = on_main! {{
        if buffer.get("localvar_loaded_history").is_some() {
            return;
        }
        let channel = match buffer.get("localvar_channelid") {
            Some(channel) => channel,
            None => {
                return;
            }
        };
        let channel = match channel.parse::<u64>() {
            Ok(v) => ChannelId(v),
            Err(_) => return,
        };
        buffer.clear();
        buffer.set("localvar_set_loaded_history", "true");
        channel
    }};

    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let http = &ctx.http;

    if let Ok(msgs) = channel.messages(http, |retriever| retriever.limit(25)) {
        on_main! {{
            for msg in msgs.iter().rev().cloned() {
                printing::print_msg(&buffer, &msg, false);
            }
        }};
    }
}

pub fn update_nick() {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let current_user = ctx.cache.read().user.clone();

    for guild in current_user
        .guilds(&ctx.http)
        .expect("Unable to fetch guilds")
    {
        // TODO: Colors?
        let nick = if let Ok(current_member) = guild.id.member(&ctx, current_user.id) {
            format!("@{}", current_member.display_name())
        } else {
            format!("@{}", current_user.name)
        };

        let channels = guild
            .id
            .channels(&ctx.http)
            .expect("Unable to fetch channels");
        for channel_id in channels.keys() {
            let string_channel = utils::buffer_id_for_channel(Some(guild.id), *channel_id);
            if let Some(buffer) = Buffer::search(&string_channel) {
                buffer.set("localvar_set_nick", &nick);
                update_bar_item("input_prompt");
            }
        }
    }
}
