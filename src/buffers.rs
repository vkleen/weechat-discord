use ffi::Buffer;
use printing;
use serenity::builder::GetMessages;
use serenity::model::prelude::*;
use serenity::CACHE;

pub fn create_buffers() {
    let current_user = CACHE.read().user.clone();

    for guild in current_user.guilds().expect("Unable to fetch guilds") {
        create_buffer_from_guild(&guild);

        // TODO: Colors?
        let nick = if let Ok(current_member) = guild.id.member(current_user.id) {
            format!("@{}", current_member.display_name())
        } else {
            format!("@{}", current_user.name)
        };
        let channels = guild.id.channels().unwrap();
        let mut channels = channels.values().collect::<Vec<_>>();
        channels.sort_by_key(|g| g.position);
        for channel in channels {
            create_buffer_from_channel(&channel, &nick)
        }
    }
}

fn create_buffer_from_guild(guild: &GuildInfo) {
    let guild_name_id = guild.id.0.to_string();
    let buffer = if let Some(buffer) = Buffer::search(&guild_name_id) {
        buffer
    } else {
        Buffer::new(&guild_name_id, |_, _| {}).unwrap()
    };
    buffer.set("short_name", &guild.name);
    buffer.set("localvar_set_type", "server");
}

fn create_buffer_from_channel(channel: &GuildChannel, nick: &str) {
    let guild_name_id = channel.guild_id.0.to_string();

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

    let name_id = channel.id.0.to_string();
    let buffer = if let Some(buffer) = Buffer::search(&name_id) {
        buffer
    } else {
        Buffer::new(&name_id, ::hook::buffer_input).unwrap()
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

// TODO: Make this nicer somehow
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

            let guild = guild_id.find().expect("No guild cache item");

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
                    if let Some((role, pos)) = member.highest_role_info() {
                        if let Some(role) = role.find() {
                            buffer.add_nicklist_group(&format!(
                                "{}|{}",
                                ::std::i64::MAX - pos,
                                role.name
                            ));
                            buffer.add_nick_to_group(member.display_name().as_ref(), &role.name)
                        } else {
                            buffer.add_nick(member.display_name().as_ref());
                        }
                    } else {
                        buffer.add_nick(member.display_name().as_ref());
                    }
                }
            }
        }
    }
}

pub fn load_history(buffer: &Buffer) {
    if let Some(channel) = buffer.get("localvar_channelid") {
        if let Some(_) = buffer.get("localvar_loaded_history") {
            return;
        }
        buffer.clear();
        buffer.set("localvar_set_loaded_history", "true");
        let channel = match channel.parse::<u64>() {
            Ok(v) => ChannelId(v),
            Err(_) => return,
        };

        let retriever = GetMessages::default().limit(25);

        if let Ok(msgs) = channel.messages(|_| retriever) {
            for msg in msgs.iter().rev().cloned() {
                printing::print_msg(&buffer, &msg, false);
            }
        }
    }
}
