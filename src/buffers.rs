use serenity::{builder::GetMessages, model::prelude::*, CACHE};
use std::collections::{HashMap, VecDeque};
use {
    ffi::{update_bar_item, Buffer},
    printing, utils,
};

pub fn create_buffers(ready_data: &Ready) {
    let current_user = CACHE.read().user.clone();

    let guilds = match current_user.guilds() {
        Ok(guilds) => guilds,
        _ => {
            on_main! {{ ::plugin_print("Error getting user guilds"); }}
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
        create_buffer_from_guild(&guild);

        // TODO: Colors?
        let nick = if let Ok(current_member) = guild.id.member(current_user.id) {
            format!("@{}", current_member.display_name())
        } else {
            format!("@{}", current_user.name)
        };
        let channels = guild.id.channels().expect("Unable to fetch channels");
        let mut channels = channels.values().collect::<Vec<_>>();
        channels.sort_by_key(|g| g.position);
        for channel in channels {
            create_buffer_from_channel(&channel, &nick)
        }
    }
}

fn create_buffer_from_guild(guild: &GuildInfo) {
    let guild_name_id = utils::buffer_id_from_guild(&guild.id);
    let buffer = on_main! {{
        if let Some(buffer) = Buffer::search(&guild_name_id) {
            buffer
        } else {
            Buffer::new(&guild_name_id, |_, _| {}).unwrap()
        }
    }};
    buffer.set("short_name", &guild.name);
    buffer.set("localvar_set_type", "server");
}

fn create_buffer_from_channel(channel: &GuildChannel, nick: &str) {
    let guild_name_id = utils::buffer_id_from_guild(&channel.guild_id);

    let current_user = CACHE.read().user.clone();
    if let Ok(perms) = channel.permissions_for(current_user.id) {
        if !perms.read_message_history() {
            return;
        }
    }

    let channel_type = match channel.kind {
        ChannelType::Category | ChannelType::Voice => return,
        ChannelType::Private => "private",
        ChannelType::Group | ChannelType::Text => "channel",
    };

    let name_id = utils::buffer_id_from_channel(&channel.id);
    on_main! {{
        let buffer = if let Some(buffer) = Buffer::search(&name_id) {
            buffer
        } else {
            Buffer::new(&name_id, ::hook::buffer_input).unwrap()
        };
        buffer.set("short_name", &channel.name);
        buffer.set("localvar_set_channelid", &name_id[1..]);
        buffer.set("localvar_set_guildid", &guild_name_id[1..]);
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
    }};
}

// TODO: Reduce code duplication
pub fn create_buffer_from_dm(channel: Channel, nick: &str, switch_to: bool) {
    let channel = match channel.private() {
        Some(chan) => chan,
        None => return,
    };
    let channel = channel.read();

    let name_id = utils::buffer_id_from_channel(&channel.id);
    on_main! {{
        let buffer = if let Some(buffer) = Buffer::search(&name_id) {
            buffer
        } else {
            Buffer::new(&name_id, ::hook::buffer_input).unwrap()
        };

        buffer.set("short_name", &channel.name());
        buffer.set("localvar_set_channelid", &name_id[1..]);
        buffer.set("localvar_set_nick", &nick);
        if switch_to {
            buffer.set("display", "1");
        }
        let title = format!("DM with {}", channel.recipient.read().name);
        buffer.set("title", &title);
    }};
}

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

    let name_id = utils::buffer_id_from_channel(&channel.channel_id);

    on_main! {{
        let buffer = if let Some(buffer) = Buffer::search(&name_id) {
            buffer
        } else {
            Buffer::new(&name_id, ::hook::buffer_input).unwrap()
        };

        buffer.set("short_name", &channel.name());
        buffer.set("localvar_set_channelid", &name_id[1..]);
        buffer.set("localvar_set_nick", &nick);
        buffer.set("title", &title);
    }};
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

    let guild = guild_id.to_guild_cached().expect("No guild cache item");

    let guild_lock = guild.read();

    // Typeck not smart enough
    let none_user: Option<UserId> = None;
    for member in guild_lock.members(None, none_user).unwrap() {
        let user_id = member.user.read().id;
        let member_perms = guild_lock.permissions_in(channel_id, user_id);
        if !member_perms.send_messages()
            || !member_perms.read_message_history()
            || !member_perms.read_messages()
        {
            continue;
        } else {
            on_main! {{
                if let Some((role, pos)) = member.highest_role_info() {
                    if let Some(role) = role.to_role_cached() {
                        let role_name = &format!("{}|{}", ::std::i64::MAX - pos, role.name);
                        if !buffer.group_exists(role_name) {
                            buffer.add_nicklist_group(role_name);
                        }
                        buffer.add_nick_to_group(member.display_name().as_ref(), &role.name)
                    } else {
                        buffer.add_nick(member.display_name().as_ref());
                    }
                } else {
                    buffer.add_nick(member.display_name().as_ref());
                }
            }};
        }
    }
}

pub fn load_history(buffer: &Buffer) {
    let channel = on_main! {{
        if buffer.get("localvar_loaded_history").is_some() {
            return;
        }
        let channel = match buffer.get("localvar_channelid") {
            Some(channel) => channel,
            None => return,
        };
        let channel = match channel.parse::<u64>() {
            Ok(v) => ChannelId(v),
            Err(_) => return,
        };
        buffer.clear();
        buffer.set("localvar_set_loaded_history", "true");
        channel
    }};

    let retriever = GetMessages::default().limit(25);

    if let Ok(msgs) = channel.messages(|_| retriever) {
        for msg in msgs.iter().rev().cloned() {
            printing::print_msg(&buffer, &msg, false);
        }
    }
}

pub fn update_nick() {
    let current_user = CACHE.read().user.clone();

    for guild in current_user.guilds().expect("Unable to fetch guilds") {
        // TODO: Colors?
        let nick = if let Ok(current_member) = guild.id.member(current_user.id) {
            format!("@{}", current_member.display_name())
        } else {
            format!("@{}", current_user.name)
        };

        let channels = guild.id.channels().expect("Unable to fetch channels");
        for channel_id in channels.keys() {
            let string_channel = utils::buffer_id_from_channel(&channel_id);
            if let Some(buffer) = Buffer::search(&string_channel) {
                buffer.set("localvar_set_nick", &nick);
                update_bar_item("input_prompt");
            }
        }
    }
}
