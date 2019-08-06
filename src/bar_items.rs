use std::borrow::Cow;
use weechat::bar::BarItem;
use weechat::Weechat;

pub struct BarHandles {
    _guild_name: BarItem,
    _channel_name: BarItem,
    _full_name: BarItem,
}

pub fn init(weechat: &Weechat) -> BarHandles {
    let _guild_name = weechat.new_bar_item("buffer_guild_name", |_, buffer| {
        buffer
            .get_localvar("guild_name")
            .map(Cow::into_owned)
            .unwrap_or_default()
            .to_owned()
    });

    let _channel_name = weechat.new_bar_item("buffer_channel_name", |_, buffer| {
        buffer
            .get_localvar("channel_name")
            .map(Cow::into_owned)
            .unwrap_or_default()
            .to_owned()
    });

    let _full_name = weechat.new_bar_item("buffer_discord_full_name", |_, buffer| {
        let guild_name = buffer.get_localvar("guild_name");
        let channel_name = buffer.get_localvar("channel_name");
        match (guild_name, channel_name) {
            // i don't think the second pattern is possible
            (Some(name), None) | (None, Some(name)) => format!("{}", name),
            (Some(guild_name), Some(channel_name)) => format!("{}:{}", guild_name, channel_name),
            (None, None) => String::new(),
        }
    });

    BarHandles {
        _guild_name,
        _channel_name,
        _full_name,
    }
}
