use crate::utils::BufferExt;
use std::borrow::Cow;
use weechat::{bar::BarItem, Weechat};

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
            let typing_events = crate::discord::TYPING_EVENTS.lock();

            if let Some(channel_id) = buffer.channel_id() {
                let guild_id = buffer.guild_id();
                let mut users = typing_events
                    .entries
                    .iter()
                    .filter(|e| e.guild_id == guild_id && e.channel_id == channel_id)
                    .map(|e| e.user_name.clone())
                    .collect::<Vec<_>>();
                users.dedup();
                let users = users.join(", ");

                if users.is_empty() {
                    "".into()
                } else {
                    format!("typing: {}", users)
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
