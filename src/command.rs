use crate::{buffers, discord, plugin_print, utils};
use serenity::model::id::ChannelId;
use weechat::{Buffer, CommandHook, Weechat};

pub fn init(weechat: &Weechat) -> CommandHook<()> {
    weechat.hook_command(
        CMD_DESCRIPTION,
        |_, buffer, args| run_command(&buffer, &args.collect::<Vec<_>>().join(" ")),
        None,
    )
}

#[derive(Clone, Copy)]
struct Args<'a> {
    args: &'a [&'a str],
    rest: &'a str,
}

fn run_command(buffer: &Buffer, cmd: &str) {
    let weechat = &buffer.get_weechat();

    let mut args: Vec<_> = cmd.split(' ').skip(1).collect();
    if args.is_empty() {
        plugin_print("see /help discord for more information");
        return;
    }
    let base = args.remove(0);
    let args = Args {
        args: &args,
        rest: &cmd["/discord ".len() + base.len()..].trim(),
    };

    match base {
        "connect" => connect(weechat),
        "disconnect" => disconnect(weechat),
        "irc-mode" => irc_mode(weechat),
        "discord-mode" => discord_mode(weechat),
        "token" => token(weechat, args),
        "autostart" => autostart(weechat),
        "noautostart" => noautostart(weechat),
        "query" => {
            crate::hook::handle_query(buffer, &format!("/{}", cmd));
        }
        "join" => join(weechat, args),
        "watch" => watch(weechat, args),
        "watched" => watched(weechat),
        "autojoin" => autojoin(weechat, args, buffer),
        "autojoined" => autojoined(weechat),
        "upload" => upload(args, buffer),
        _ => {
            plugin_print("Unknown command");
        }
    }
}

fn connect(weechat: &Weechat) {
    match weechat.get_plugin_option("token") {
        Some(t) => {
            if crate::discord::DISCORD.lock().is_none() {
                crate::discord::init(weechat, &t, crate::utils::get_irc_mode(weechat));
            } else {
                plugin_print("Already connected");
            }
        }
        None => {
            plugin_print("Error: plugins.var.weecord.token unset. Run:");
            plugin_print("/discord token 123456789ABCDEF");
        }
    };
}

fn disconnect(_weechat: &Weechat) {
    let mut discord = crate::discord::DISCORD.lock();
    if discord.is_some() {
        if let Some(discord) = discord.take() {
            discord.shutdown();
        };
        plugin_print("Disconnected");
    } else {
        plugin_print("Already disconnected");
    }
}

fn irc_mode(weechat: &Weechat) {
    if crate::utils::get_irc_mode(weechat) {
        plugin_print("irc-mode already enabled")
    } else {
        user_set_option(weechat, "irc_mode", "true");
        plugin_print("irc-mode enabled")
    }
}

fn discord_mode(weechat: &Weechat) {
    if !crate::utils::get_irc_mode(weechat) {
        plugin_print("discord-mode already enabled")
    } else {
        user_set_option(weechat, "irc_mode", "false");
        plugin_print("discord-mode enabled")
    }
}

fn token(weechat: &Weechat, args: Args) {
    if args.args.is_empty() {
        plugin_print("token requires an argument");
    } else {
        user_set_option(weechat, "token", args.rest.trim_matches('"'));
        plugin_print("Set Discord token");
    }
}

fn autostart(weechat: &Weechat) {
    weechat.set_plugin_option("autostart", "true");
    plugin_print("Discord will now load on startup");
}

fn noautostart(weechat: &Weechat) {
    weechat.set_plugin_option("autostart", "false");
    plugin_print("Discord will not load on startup");
}

fn join(_weechat: &Weechat, args: Args) {
    if args.args.is_empty() {
        plugin_print("join requires an guild name and channel name");
    } else {
        let mut args = args.args.iter();
        let guild_name = match args.next() {
            Some(g) => g,
            None => return,
        };
        let channel_name = match args.next() {
            Some(c) => c,
            None => return,
        };

        let ctx = match discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };

        if let Some((guild, channel)) =
            crate::utils::search_channel(&ctx.cache, guild_name, channel_name)
        {
            let guild = guild.read();
            buffers::create_guild_buffer(guild.id, &guild.name);
            // TODO: Add correct nick handling
            buffers::create_buffer_from_channel(
                &ctx.cache,
                &guild.name,
                &channel.read(),
                &ctx.cache.read().user.name,
                false,
            );
            return;
        }
        plugin_print("Couldn't find channel")
    }
}

fn watch(weechat: &Weechat, args: Args) {
    if args.args.is_empty() {
        plugin_print("watch requires a guild name and channel name");
    } else {
        let mut args = args.args.iter().filter(|i| !i.is_empty());
        let guild_name = match args.next() {
            Some(g) => g,
            None => return,
        };
        let channel_name = args.next();

        let ctx = match discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };

        let new_channel_id = if let Some(channel_name) = channel_name {
            if let Some((guild, channel)) =
                crate::utils::search_channel(&ctx.cache, guild_name, channel_name)
            {
                crate::utils::unique_id(Some(guild.read().id), channel.read().id)
            } else {
                plugin_print("Unable to find server and channel");
                return;
            }
        } else if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
            crate::utils::unique_guild_id(guild.read().id)
        } else {
            plugin_print("Unable to find server");
            return;
        };
        let new_watched =
            if let Some(watched_channels) = weechat.get_plugin_option("watched_channels") {
                // dedup items
                let mut channels: Vec<_> = watched_channels
                    .split(',')
                    .filter(|i| !i.is_empty())
                    .collect();
                channels.push(&new_channel_id);

                channels.dedup();
                channels.join(",")
            } else {
                new_channel_id
            };
        weechat.set_plugin_option("watched_channels", &new_watched);
        if let Some(channel_name) = channel_name {
            plugin_print(&format!("Now watching {} in {}", guild_name, channel_name))
        } else {
            plugin_print(&format!("Now watching all of {}", guild_name))
        }
    }
}

fn watched(weechat: &Weechat) {
    weechat.print("");
    let mut channels = Vec::new();
    let mut guilds = Vec::new();

    let ctx = match discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    match weechat.get_plugin_option("watched_channels") {
        Some(watched) => {
            let items = watched.split(',').filter_map(utils::parse_id);
            for i in items {
                match i {
                    utils::GuildOrChannel::Guild(guild) => guilds.push(guild),
                    utils::GuildOrChannel::Channel(guild, channel) => {
                        channels.push((guild, channel))
                    }
                }
            }
        }
        None => {
            plugin_print("Unable to get watched channels");
            return;
        }
    };

    weechat.print(&format!("Watched Servers: ({})", guilds.len()));
    for guild in guilds {
        if let Some(guild) = guild.to_guild_cached(ctx) {
            weechat.print(&format!("  {}", guild.read().name));
        }
    }

    weechat.print(&format!("Watched Channels: ({})", channels.len()));
    for (guild, channel) in channels {
        if let Ok(channel) = channel.to_channel(ctx) {
            let channel_name = utils::channel_name(&channel);
            if let Some(guild) = guild {
                let guild_name = if let Some(guild) = guild.to_guild_cached(&ctx) {
                    guild.read().name.to_owned()
                } else {
                    guild.0.to_string()
                };
                weechat.print(&format!("  {}: {}", guild_name, channel_name));
            } else {
                weechat.print(&format!("  {}", channel_name));
            }
        } else {
            weechat.print(&format!("  {:?} {:?}", guild, channel));
        }
    }
}

fn autojoin(weechat: &Weechat, args: Args, buffer: &Buffer) {
    if args.args.is_empty() {
        plugin_print("autojoin requires a guild name and channel name");
    } else {
        let mut opts = args.args.iter().filter(|i| !i.is_empty());
        let guild_name = match opts.next() {
            Some(g) => g,
            None => return,
        };
        let channel_name = opts.next();

        let ctx = match discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };

        let new_channel_id = if let Some(channel_name) = channel_name {
            if let Some((guild, channel)) =
                crate::utils::search_channel(&ctx.cache, guild_name, channel_name)
            {
                crate::utils::unique_id(Some(guild.read().id), channel.read().id)
            } else {
                plugin_print("Unable to find server and channel");
                return;
            }
        } else if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
            crate::utils::unique_guild_id(guild.read().id)
        } else {
            plugin_print("Unable to find server");
            return;
        };
        let new_autojoined =
            if let Some(autojoined_channels) = weechat.get_plugin_option("autojoin_channels") {
                // dedup items
                let mut channels: Vec<_> = autojoined_channels
                    .split(',')
                    .filter(|i| !i.is_empty())
                    .collect();
                channels.push(&new_channel_id);

                channels.dedup();
                channels.join(",")
            } else {
                new_channel_id
            };
        weechat.set_plugin_option("autojoin_channels", &new_autojoined);

        if let Some(channel_name) = channel_name {
            plugin_print(&format!(
                "Now autojoining {} in {}",
                guild_name, channel_name
            ));
            run_command(buffer, &format!("/discord join {}", args.rest));
        } else {
            plugin_print(&format!("Now autojoining all channels in {}", guild_name))
        }
    }
}

fn autojoined(weechat: &Weechat) {
    weechat.print("");
    let mut channels = Vec::new();
    let mut guilds = Vec::new();

    let ctx = match discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    match weechat.get_plugin_option("autojoin_channels") {
        Some(watched) => {
            let items = watched.split(',').filter_map(utils::parse_id);
            for i in items {
                match i {
                    utils::GuildOrChannel::Guild(guild) => guilds.push(guild),
                    utils::GuildOrChannel::Channel(guild, channel) => {
                        channels.push((guild, channel))
                    }
                }
            }
        }
        None => {
            plugin_print("Unable to get autojoin channels");
            return;
        }
    };

    weechat.print(&format!("Autojoin Servers: ({})", guilds.len()));
    for guild in guilds {
        if let Some(guild) = guild.to_guild_cached(ctx) {
            weechat.print(&format!("  {}", guild.read().name));
        }
    }

    weechat.print(&format!("Autojoin Channels: ({})", channels.len()));
    for (guild, channel) in channels {
        if let Ok(channel) = channel.to_channel(ctx) {
            let channel_name = utils::channel_name(&channel);
            if let Some(guild) = guild {
                let guild_name = if let Some(guild) = guild.to_guild_cached(&ctx) {
                    guild.read().name.to_owned()
                } else {
                    guild.0.to_string()
                };
                weechat.print(&format!("  {}: {}", guild_name, channel_name));
            } else {
                weechat.print(&format!("  {}", channel_name));
            }
        } else {
            weechat.print(&format!("  {:?} {:?}", guild, channel));
        }
    }
}

fn upload(args: Args, buffer: &Buffer) {
    if args.args.is_empty() {
        plugin_print("upload requires an argument");
    } else {
        let mut file = args.rest.to_owned();
        // TODO: Find a better way to expand paths
        if file.starts_with("~/") {
            let rest: String = file.chars().skip(2).collect();
            let dir = match dirs::home_dir() {
                Some(dir) => dir.to_string_lossy().into_owned(),
                None => ".".to_owned(),
            };
            file = format!("{}/{}", dir, rest);
        }
        let full = match std::fs::canonicalize(file) {
            Ok(f) => f.to_string_lossy().into_owned(),
            Err(e) => {
                plugin_print(&format!("Unable to resolve file path: {}", e));
                return;
            }
        };
        let full = full.as_str();
        // TODO: Check perms and file size
        let channel = match buffer.get_localvar("channelid") {
            Some(channel) => channel,
            None => return,
        };
        let channel = match channel.parse::<u64>() {
            Ok(v) => ChannelId(v),
            Err(_) => return,
        };
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };
        match channel.send_files(ctx, vec![full], |m| m) {
            Ok(_) => plugin_print("File uploaded successfully"),
            Err(e) => {
                if let serenity::Error::Model(serenity::model::ModelError::MessageTooLong(_)) = e {
                    plugin_print("File too large to upload");
                }
            }
        };
    }
}

fn user_set_option(weechat: &Weechat, name: &str, value: &str) {
    let before = weechat.get_plugin_option(name);
    let changed = weechat.set_plugin_option(name, value);

    use weechat::OptionChanged::*;
    let msg = match (changed, before) {
        (Changed, Some(before)) => format!(
            "option {} successfully changed from {} to {}",
            name, before, value
        ),
        (Changed, None) | (Unchanged, None) => {
            format!("option {} successfully set to {}", name, value)
        }
        (Unchanged, Some(before)) => format!("option {} already contained {}", name, before),
        (NotFound, _) => format!("option {} not found", name),
        (Error, Some(before)) => format!(
            "error when setting option {} to {} (was {})",
            name, value, before
        ),
        (Error, _) => format!("error when setting option {} to {}", name, value),
    };

    plugin_print(&msg);
}

const CMD_DESCRIPTION: weechat::CommandDescription = weechat::CommandDescription {
    name: "discord",
    description: "\
Discord from the comfort of your favorite command-line IRC client!
Source code available at https://github.com/terminal-discord/weechat-discord
Originally by https://github.com/khyperia/weechat-discord
Options used:
plugins.var.weecord.token = <discord_token>
plugins.var.weecord.rename.<id> = <string>
plugins.var.weecord.autostart = <bool>
plugins.var.weecord.irc_mode = <bool>",
    args: "\
connect
disconnect
join
query
watch
autojoin
watched
autojoined
irc-mode
discord-mode
autostart
noautostart
token <token>
upload <file>",
    args_description:
"connect: sign in to discord and open chat buffers
disconnect: sign out of Discord
join: join a channel in irc mode by providing guild name and channel name
query: open a dm with a user (for when there are no discord buffers open)
irc-mode: enable irc-mode, meaning that weecord will not load all channels like the official client
discord-mode: enable discord-mode, meaning all available channels and guilds will be added to the buflist
watch: Automatically open a buffer when a message is received in a guild or channel
autojoin: Automatically open a channel or entire guild when discord connects
watched: List watched guilds and channels
autojoined: List autojoined guilds and channels
autostart: automatically sign into discord on start
noautostart: disable autostart
token: set Discord login token
upload: upload a file to the current channel
Examples:
  /discord token 123456789ABCDEF
  /discord connect
  /discord autostart
  /discord disconnect
  /discord upload file.txt
",
    completion:
"connect || \
disconnect || \
query %(weecord_dm_completion) || \
watch %(weecord_guild_completion) %(weecord_channel_completion) || \
watched || \
autojoined || \
autojoin %(weecord_guild_completion) %(weecord_channel_completion) || \
irc-mode || \
discord-mode || \
token || \
autostart || \
noautostart || \
upload %(filename) || \
join %(weecord_guild_completion) %(weecord_channel_completion)",
};
