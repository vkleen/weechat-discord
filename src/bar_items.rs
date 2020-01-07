use crate::utils::BufferExt;
use serenity::model::id::{ChannelId, GuildId};
use std::borrow::Cow;
use weechat::{bar::BarItem, ConfigOption, Weechat};

pub struct BarHandles {
    _guild_name: BarItem<()>,
    _channel_name: BarItem<()>,
    _full_name: BarItem<()>,
    _typing_indicator: BarItem<()>,
}

pub fn init(weechat: &Weechat) -> BarHandles {
    let _guild_name = weechat.new_bar_item(
        "buffer_guild_name",
        |_, _, buffer| {
            buffer
                .get_localvar("guild_name")
                .map(Cow::into_owned)
                .unwrap_or_default()
        },
        None,
    );

    let _channel_name = weechat.new_bar_item(
        "buffer_channel_name",
        |_, _, buffer| {
            buffer
                .get_localvar("channel_name")
                .map(Cow::into_owned)
                .unwrap_or_default()
        },
        None,
    );

    let _full_name = weechat.new_bar_item(
        "buffer_discord_full_name",
        |_, _, buffer| {
            let guild_name = buffer.get_localvar("guild_name");
            let channel_name = buffer.get_localvar("channel_name");
            match (guild_name, channel_name) {
                // i don't think the second pattern is possible
                (Some(name), None) | (None, Some(name)) => format!("{}", name),
                (Some(guild_name), Some(channel_name)) => {
                    format!("{}:{}", guild_name, channel_name)
                },
                (None, None) => String::new(),
            }
        },
        None,
    );

    let _typing_indicator = weechat.new_bar_item(
        "discord_typing",
        |_, _, buffer| {
            if let Some(channel_id) = buffer.channel_id() {
                let weechat = buffer.get_weechat();
                let config = &crate::upgrade_plugin(&weechat).config;
                let max_users = config.user_typing_list_max.value() as usize;
                let expanded = config.user_typing_list_expanded.value();
                let guild_id = buffer.guild_id();

                if expanded {
                    expanded_typing_list(channel_id, guild_id, max_users)
                } else {
                    terse_typing_list(channel_id, guild_id, max_users)
                }
            } else {
                "".into()
            }
        },
        None,
    );

    BarHandles {
        _guild_name,
        _channel_name,
        _full_name,
        _typing_indicator,
    }
}

fn terse_typing_list(channel_id: ChannelId, guild_id: Option<GuildId>, max_names: usize) -> String {
    let (head, has_more) = get_users_for_typing_list(channel_id, guild_id, max_names);

    let mut users = head.join(", ");
    if has_more {
        users = users + ", ...";
    }
    if users.is_empty() {
        "".into()
    } else {
        format!("typing: {}", users)
    }
}

fn expanded_typing_list(
    channel_id: ChannelId,
    guild_id: Option<GuildId>,
    max_names: usize,
) -> String {
    let (head, has_more) = get_users_for_typing_list(channel_id, guild_id, max_names);

    if head.is_empty() {
        "".into()
    } else if has_more {
        "Several people are typing...".into()
    } else if head.len() == 1 {
        format!("{} is typing", head[0])
    } else {
        let prefix = &head[..head.len() - 1];
        format!(
            "{} and {} are typing",
            prefix.join(", "),
            head[head.len() - 1]
        )
    }
}

fn get_users_for_typing_list(
    channel_id: ChannelId,
    guild_id: Option<GuildId>,
    max_names: usize,
) -> (Vec<String>, bool) {
    let mut users = crate::discord::TYPING_EVENTS
        .lock()
        .entries
        .iter()
        .filter(|e| e.guild_id == guild_id && e.channel_id == channel_id)
        .map(|e| e.user_name.clone())
        .collect::<Vec<_>>();
    users.dedup();
    let (head, has_more) = if users.len() > max_names {
        (&users[..max_names], true)
    } else {
        (&users[..], false)
    };
    (head.to_vec(), has_more)
}
