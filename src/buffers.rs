use crate::sync::on_main_blocking;
use crate::{on_main, utils};
use serenity::{cache::CacheRwLock, model::prelude::*, prelude::*};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use weechat::{Buffer, Weechat};

const OFFLINE_GROUP_NAME: &str = "99999|Offline";
const ONLINE_GROUP_NAME: &str = "99998|Online";
const BOT_GROUP_NAME: &str = "99999|Bot";

pub fn create_buffers(ready_data: &Ready) {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let current_user = ctx.cache.read().user.clone();

    let guilds = match current_user.guilds(ctx) {
        Ok(guilds) => guilds,
        Err(e) => {
            crate::plugin_print(&format!("Error getting user guilds: {:?}", e));
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
        sorted_guilds.push_front(guild);
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
        let nick = if let Ok(current_member) = guild.id.member(ctx, current_user.id) {
            format!("@{}", current_member.display_name())
        } else {
            format!("@{}", current_user.name)
        };
        let channels = guild.id.channels(ctx).expect("Unable to fetch channels");
        let mut channels = channels.values().collect::<Vec<_>>();
        channels.sort_by_key(|g| g.position);
        for channel in channels {
            let is_muted =
                guild_muted || channel_muted.get(&channel.id).cloned().unwrap_or_default();
            create_buffer_from_channel(&ctx.cache, &guild.name, &channel, &nick, is_muted);
        }
    }
}

// TODO: Merge these functions
// Flesh this out
pub fn create_autojoin_buffers(_ready: &Ready) {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    let current_user = ctx.cache.read().user.clone();

    let autojoined_items = on_main_blocking(move |weechat| {
        weechat
            .get_plugin_option("autojoin_channels")
            .map(ToOwned::to_owned)
    });

    let autojoin_items: String = match autojoined_items {
        Some(items) => items,
        None => return,
    };

    let autojoin_items = autojoin_items
        .split(',')
        .filter(|i| !i.is_empty())
        .filter_map(utils::parse_id);

    // flatten guilds into channels
    let channels = utils::flatten_guilds(&ctx, &autojoin_items.collect::<Vec<_>>());

    create_buffers_from_flat_items(&ctx, &current_user, &channels);
}

pub fn create_buffers_from_flat_items(
    ctx: &Context,
    current_user: &CurrentUser,
    channels: &[(Option<GuildId>, ChannelId)],
) {
    // TODO: Flatten and iterate by guild, then channel
    for (guild_id, channel_id) in channels {
        if let Some(guild_id) = guild_id {
            let guild = match guild_id.to_guild_cached(&ctx.cache) {
                Some(guild) => guild,
                None => continue,
            };
            let guild = guild.read();

            // TODO: Colors?
            let nick = if let Ok(current_member) = guild.id.member(ctx, current_user.id) {
                format!("@{}", current_member.display_name())
            } else {
                format!("@{}", current_user.name)
            };

            create_guild_buffer(guild.id, &guild.name);
            // TODO: Muting
            let () = on_main_blocking(move |_| {
                let ctx = match crate::discord::get_ctx() {
                    Some(ctx) => ctx,
                    _ => return,
                };

                let guild = match guild_id.to_guild_cached(&ctx.cache) {
                    Some(guild) => guild,
                    None => return,
                };
                let guild = guild.read();

                let channel = match guild.channels.get(&channel_id) {
                    Some(channel) => channel,
                    None => return,
                };
                let channel = channel.read();

                create_buffer_from_channel(&ctx.cache, &guild.name, &channel, &nick, false)
            });
        }
    }
}

fn find_or_make_buffer(weechat: &Weechat, name: &str) -> Buffer {
    if let Some(buffer) = weechat.buffer_search("weecord", name) {
        buffer
    } else {
        weechat.buffer_new::<(), ()>(
            name,
            Some(|_, b, i| crate::hook::buffer_input(b, &i)),
            None,
            None,
            None,
        )
    }
}

pub fn create_guild_buffer(id: GuildId, name: &str) {
    let guild_name_id = utils::buffer_id_for_guild(id);
    let _: () = on_main_blocking(move |weechat| {
        let buffer = find_or_make_buffer(&weechat, &guild_name_id);

        buffer.set_localvar("guild_name", name);
        buffer.set_short_name(name);
        buffer.set_localvar("guildid", &id.0.to_string());
        buffer.set_localvar("type", "server");
    });
}

pub fn create_buffer_from_channel(
    cache: &CacheRwLock,
    guild_name: &str,
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

    let _: () = on_main_blocking(|weechat| {
        let buffer = find_or_make_buffer(&weechat, &name_id);

        buffer.set_short_name(&channel.name);

        buffer.set_localvar("channelid", &channel.id.0.to_string());
        buffer.set_localvar("guildid", &channel.guild_id.0.to_string());
        buffer.set_localvar("channel_name", &channel.name);
        buffer.set_localvar("guild_name", guild_name);
        buffer.set_localvar("type", channel_type);
        buffer.set_localvar("nick", &nick);
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
        buffer.set_title(&title);
        buffer.set_localvar("muted", &(muted as u8).to_string());
    });
}

// TODO: Reduce code duplication
pub fn create_buffer_from_dm(
    weechat: &crate::Weechat,
    channel: Channel,
    nick: &str,
    switch_to: bool,
) {
    let channel = match channel.private() {
        Some(chan) => chan,
        None => return,
    };
    let channel = channel.read();

    let name_id = utils::buffer_id_for_channel(None, channel.id);
    let buffer = find_or_make_buffer(&weechat, &name_id);

    buffer.set_short_name(&channel.name());
    buffer.set_localvar("channelid", &channel.id.0.to_string());
    buffer.set_localvar("nick", &nick);
    if switch_to {
        buffer.switch_to();
    }
    let title = format!("DM with {}", channel.recipient.read().name);
    buffer.set_title(&title);
}

pub fn create_buffer_from_group(weechat: &Weechat, channel: Channel, nick: &str) {
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

    let buffer = find_or_make_buffer(weechat, &name_id);

    buffer.set_short_name(&channel.name());
    buffer.set_localvar("channelid", &channel.channel_id.0.to_string());
    buffer.set_localvar("nick", &nick);
    buffer.set_title(&title);
}

pub fn load_history(buffer: &weechat::Buffer) {
    if buffer.get_localvar("loaded_history").is_some() {
        return;
    }

    let channel = match buffer.get_localvar("channelid") {
        Some(ch) => ch,
        None => return,
    };
    let channel = match channel.parse::<u64>() {
        Ok(v) => ChannelId(v),
        Err(_) => return,
    };
    buffer.clear();
    buffer.set_localvar("loaded_history", "true");

    let guarded_buffer = buffer.seal();

    std::thread::spawn(move || {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };

        if let Ok(msgs) = channel.messages(ctx, |retriever| retriever.limit(25)) {
            on_main(move |weechat| {
                let buf = guarded_buffer.unseal(&weechat);
                for msg in msgs.into_iter().rev() {
                    crate::printing::print_msg(&weechat, &buf, &msg, false);
                }
            });
        }
    });
}

// TODO: Make this nicer somehow
// TODO: Refactor this to use `?`
pub fn load_nicks(buffer: &Buffer) {
    if buffer.get_localvar("loaded_nicks").is_some() {
        return;
    }

    let guild_id = match buffer.get_localvar("guildid") {
        Some(guild_id) => guild_id,
        None => return,
    };

    let channel_id = match buffer.get_localvar("channelid") {
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

    buffer.set_localvar("loaded_nicks", "true");
    buffer.enable_nicklist();

    let use_presence = buffer
        .get_weechat()
        .get_plugin_option("use_presence")
        .map(|o| o == "true")
        .unwrap_or(false);

    let guarded_buffer = buffer.seal();

    std::thread::spawn(move || {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };

        let guild = guild_id.to_guild_cached(ctx).expect("No guild cache item");

        let current_user = ctx.cache.read().user.id;

        // Typeck not smart enough
        let none_user: Option<UserId> = None;
        // TODO: What to do with more than 1000 members?
        let members = guild.read().members(ctx, Some(1000), none_user).unwrap();

        drop(guild);

        let _: () = on_main_blocking(move |weechat| {
            let ctx = match crate::discord::get_ctx() {
                Some(ctx) => ctx,
                _ => return,
            };

            let buffer = guarded_buffer.unseal(&weechat);
            let guild = guild_id.to_guild_cached(ctx).expect("No guild cache item");

            for member in members {
                add_member_to_nicklist(
                    &ctx,
                    &buffer,
                    &channel_id,
                    &guild,
                    &member,
                    current_user,
                    use_presence,
                );
            }
        });
    });
}

fn add_member_to_nicklist(
    ctx: &Context,
    buffer: &Buffer,
    channel_id: &ChannelId,
    guild: &Arc<RwLock<Guild>>,
    member: &Member,
    current_user: UserId,
    use_presence: bool,
) {
    let user = member.user.read();
    // the current user does not seem to usually have a presence, assume they are online
    let online = if !use_presence {
        // Dont do the lookup
        false
    } else if user.id == current_user {
        true
    } else {
        let cache = ctx.cache.read();
        let presence = cache.presences.get(&member.user_id());
        presence
            .map(|p| utils::status_is_online(p.status))
            .unwrap_or(false)
    };
    let member_perms = guild.read().permissions_in(channel_id, user.id);
    // A pretty accurate method of checking if a user is "in" a channel
    if !member_perms.read_message_history() || !member_perms.read_messages() {
        return;
    }

    let role_name;
    let role_color;
    // TODO: Change offline/online color somehow?
    if user.bot {
        role_name = BOT_GROUP_NAME.to_owned();
        role_color = "gray".to_string();
    } else if !online && use_presence {
        role_name = OFFLINE_GROUP_NAME.to_owned();
        role_color = "grey".to_string();
    } else if let Some((highest_hoisted, highest)) = utils::find_highest_roles(&ctx.cache, &member)
    {
        role_name = format!(
            "{}|{}",
            99999 - highest_hoisted.position,
            highest_hoisted.name
        );
        role_color = crate::utils::rgb_to_ansi(highest.colour).to_string();
    } else {
        // Can't find a role, add user to generic bucket
        if use_presence {
            if online {
                role_name = ONLINE_GROUP_NAME.to_owned();
            } else {
                role_name = OFFLINE_GROUP_NAME.to_owned();
            }
            role_color = "grey".to_string();
        } else {
            buffer.add_nick(
                weechat::NickArgs {
                    name: member.display_name().as_ref(),
                    ..Default::default()
                },
                None,
            );
            return;
        }
    }
    let group = match buffer.search_nicklist_group(&role_name) {
        Some(group) => group,
        None => buffer.add_group(&role_name, &role_color, true, None),
    };
    buffer.add_nick(
        weechat::NickArgs {
            name: member.display_name().as_ref(),
            ..Default::default()
        },
        Some(&group),
    );
}

pub fn update_nick() {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let current_user = ctx.cache.read().user.clone();

    for guild in current_user.guilds(ctx).expect("Unable to fetch guilds") {
        // TODO: Colors?
        let nick = if let Ok(current_member) = guild.id.member(ctx, current_user.id) {
            format!("@{}", current_member.display_name())
        } else {
            format!("@{}", current_user.name)
        };

        let channels = guild.id.channels(ctx).expect("Unable to fetch channels");
        on_main(move |weechat| {
            for channel_id in channels.keys() {
                let string_channel = utils::buffer_id_for_channel(Some(guild.id), *channel_id);
                let nick = nick.to_owned();
                if let Some(buffer) = weechat.buffer_search("weecord", &string_channel) {
                    buffer.set_localvar("nick", &nick);
                    weechat.update_bar_item("input_prompt");
                }
            }
        })
    }
}

pub fn update_member_nick(old: &Option<Member>, new: &Member) {
    let old_nick = match old.as_ref().map(|old| old.display_name()) {
        Some(old) => old,
        None => {
            // TODO: Rebuild entire nicklist?
            return;
        }
    };
    let new_nick = new.display_name();
    let new = new.clone();
    let guild_id = new.guild_id;

    if old_nick != new_nick {
        let old_nick = old_nick.to_owned().to_string();
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };

        let channels = guild_id.channels(ctx).expect("Unable to fetch channels");

        on_main(move |weechat| {
            let ctx = match crate::discord::get_ctx() {
                Some(ctx) => ctx,
                _ => return,
            };
            let current_user = ctx.cache.read().user.id;
            for channel_id in channels.keys() {
                let string_channel = utils::buffer_id_for_channel(Some(guild_id), *channel_id);
                if let Some(buffer) = weechat.buffer_search("weecord", &string_channel) {
                    if let Some(nick) = buffer.search_nick(&old_nick, None) {
                        nick.remove();
                        if let Some(guild) = guild_id.to_guild_cached(&ctx) {
                            add_member_to_nicklist(
                                &ctx,
                                &buffer,
                                channel_id,
                                &guild,
                                &new,
                                current_user,
                                false,
                            );
                        }
                    }
                }
            }
        })
    }
}
