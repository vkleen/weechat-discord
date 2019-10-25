use weechat::{BooleanOption, ConfigOption, ConfigSectionInfo, StringOption, Weechat};

use crate::utils;
use crate::utils::GuildOrChannel;

pub struct Config {
    pub token: StringOption,
    pub watched_channels: StringOption,
    pub autojoin_channels: StringOption,
    pub autostart: BooleanOption,
    pub use_presence: BooleanOption,
    pub send_typing_events: BooleanOption,
    pub typing_messages: BooleanOption,
    pub irc_mode: BooleanOption,
    pub config: weechat::Config<()>,
}

pub fn init(weechat: &Weechat) -> Config {
    let mut config = weechat.config_new("weecord", None, None);

    let section_info: ConfigSectionInfo<()> = ConfigSectionInfo {
        name: "main",
        ..Default::default()
    };

    let section = config.new_section(section_info);

    let token = section.new_string_option(
        "token",
        "Discord auth token. Supports secure data",
        "",
        "",
        false,
        None,
        None::<()>,
    );

    let watched_channels = section.new_string_option(
        "watched_channels",
        "List of channels to open when a message is received",
        "",
        "",
        false,
        None,
        None::<()>,
    );

    let autojoin_channels = section.new_string_option(
        "autojoin_channels",
        "List of channels to automatically open on connecting (irc mode only)",
        "",
        "",
        false,
        None,
        None::<()>,
    );

    let autostart = section.new_boolean_option(
        "autostart",
        "Automatically connect to Discord when weechat starts",
        false,
        false,
        false,
        None,
        None::<()>,
    );

    let use_presence = section.new_boolean_option(
        "use_presence",
        "Show the presence of other users in the nicklist",
        false,
        false,
        false,
        None,
        None::<()>,
    );

    let send_typing_events = section.new_boolean_option(
        "send_typing_events",
        "Send typing events to the channel",
        false,
        false,
        false,
        None,
        None::<()>,
    );

    let typing_messages = section.new_boolean_option(
        "typing_messages",
        "Print a message when someone in a channel is typing",
        false,
        false,
        false,
        None,
        None::<()>,
    );

    let irc_mode = section.new_boolean_option(
        "irc_mode",
        r#"Enable "IRC-Mode" where only the channels you choose will be automatically joined"#,
        false,
        false,
        false,
        None,
        None::<()>,
    );

    config.read();

    Config {
        token,
        watched_channels,
        autojoin_channels,
        autostart,
        use_presence,
        send_typing_events,
        typing_messages,
        irc_mode,
        config,
    }
}

impl Config {
    pub fn autojoin_channels(&self) -> Vec<GuildOrChannel> {
        self.autojoin_channels
            .value()
            .split(',')
            .filter(|i| !i.is_empty())
            .filter_map(utils::parse_id)
            .collect()
    }

    pub fn watched_channels(&self) -> Vec<GuildOrChannel> {
        self.watched_channels
            .value()
            .split(',')
            .filter(|i| !i.is_empty())
            .filter_map(utils::parse_id)
            .collect()
    }
}
