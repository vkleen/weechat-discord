use crate::{
    buffers, discord, on_main_blocking, plugin_print, upgrade_plugin, utils,
    utils::{BufferExt, ChannelExt, GuildOrChannel},
    weechat_utils::MessageManager,
    Discord,
};
use lazy_static::lazy_static;
use parking_lot::Mutex;
use serenity::model::{gateway::Activity, user::OnlineStatus};
use std::{borrow::Cow, collections::VecDeque, sync::Arc};
use weechat::{Buffer, CommandHook, ConfigOption, ReturnCode, Weechat};

lazy_static! {
    // Tracks the last set status for use in setting the current game presence
    pub static ref LAST_STATUS: Arc<Mutex<OnlineStatus>> = Arc::new(Mutex::new(OnlineStatus::Online));
}

pub fn init(weechat: &Weechat) -> Vec<CommandHook<()>> {
    let mut hooks = Vec::new();
    hooks.push(weechat.hook_command(
        CMD_DESCRIPTION,
        |_, buffer, args| run_command(&buffer, &args.collect::<Vec<_>>().join(" ")),
        None,
    ));
    hooks.push(weechat.hook_command(
        weechat::CommandDescription {
            name: "me",
            description: "Send an italicized message to Discord.",
            args: "",
            args_description: "",
            completion: "",
        },
        |_, buffer, args| {
            run_command(
                &buffer,
                &("/discord me ".to_string() + &args.skip(1).collect::<Vec<_>>().join(" ")),
            )
        },
        None,
    ));
    return hooks;
}

#[derive(Clone, Debug)]
pub struct Args<'a> {
    pub base: &'a str,
    pub args: VecDeque<&'a str>,
    pub rest: &'a str,
}

impl<'a> Args<'a> {
    pub fn from_cmd(cmd: &'a str) -> Args<'a> {
        let mut args: VecDeque<_> = cmd.split(' ').skip(1).collect();
        if args.is_empty() {
            return Args {
                base: "",
                args: VecDeque::new(),
                rest: "",
            };
        }
        let base = args.remove(0).unwrap();
        Args {
            base,
            args,
            rest: &cmd["/discord ".len() + base.len()..].trim(),
        }
    }
}

fn run_command(buffer: &Buffer, cmd: &str) {
    let weechat = buffer.get_weechat();
    let weecord = upgrade_plugin(&weechat);

    let args = Args::from_cmd(cmd);

    if args.base.is_empty() {
        plugin_print("no action provided.");
        plugin_print("see /help discord for more information");
        return;
    }

    match args.base {
        "connect" => weecord.connect(),
        "disconnect" => disconnect(weecord),
        "irc-mode" => irc_mode(weecord),
        "discord-mode" => discord_mode(weecord),
        "token" => token(weecord, &args),
        "autostart" => autostart(weecord),
        "noautostart" => noautostart(weecord),
        "query" => {
            crate::hook::handle_query(&args);
        },
        "join" => {
            join(weecord, &args, true);
        },
        "watch" => watch(weecord, &args),
        "nowatch" => nowatch(weecord, &args),
        "watched" => watched(weecord),
        "autojoin" => autojoin(weecord, &args, buffer),
        "noautojoin" => noautojoin(weecord, &args),
        "autojoined" => autojoined(weecord),
        "status" => status(&args),
        "pins" | "pinned" => pins(weecord, buffer),
        "game" => game(&args),
        "upload" => upload(&args, buffer),
        "me" | "tableflip" | "unflip" | "shrug" | "spoiler" => {
            discord_fmt(args.base, args.rest, buffer)
        },
        "rehistory" => {
            let buffer_name = buffer.get_name().to_string();
            if let Some(buffer) = weecord.buffer_manager.get_buffer(&buffer_name) {
                rehistory(weecord, &args, &*buffer);
            }
        },
        _ => {
            plugin_print("Unknown command");
        },
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
        let weecord = crate::upgrade_plugin(weechat);
        let before = weecord.config.irc_mode.value();
        let change = weecord.config.irc_mode.set(true);
        format_option_change("irc_mode", "true", Some(&before), change);
        plugin_print("irc-mode enabled")
    }
}

fn discord_mode(weechat: &Weechat) {
    if !crate::utils::get_irc_mode(weechat) {
        plugin_print("discord-mode already enabled")
    } else {
        let weecord = crate::upgrade_plugin(weechat);
        let before = weecord.config.irc_mode.value();
        let change = weecord.config.irc_mode.set(false);
        format_option_change("irc_mode", "false", Some(&before), change);
        plugin_print("discord-mode enabled")
    }
}

fn token(weechat: &Weechat, args: &Args) {
    if args.args.is_empty() {
        plugin_print("token requires an argument");
    } else {
        let weecord = crate::upgrade_plugin(weechat);
        let new_value = args.rest.trim_matches('"');
        weecord.config.token.set(new_value);

        plugin_print("Set Discord token");
    }
}

fn autostart(weechat: &Weechat) {
    crate::upgrade_plugin(weechat).config.autostart.set(true);
    plugin_print("Discord will now load on startup");
}

fn noautostart(weechat: &Weechat) {
    crate::upgrade_plugin(weechat).config.autostart.set(false);
    plugin_print("Discord will not load on startup");
}

pub(crate) fn join(_weechat: &Weechat, args: &Args, verbose: bool) -> ReturnCode {
    if args.args.is_empty() && verbose {
        plugin_print("join requires an guild name and optional channel name");
        ReturnCode::Error
    } else {
        let mut args = args.args.iter();
        let guild_name = match args.next() {
            Some(g) => g,
            None => return ReturnCode::Error,
        };
        let channel_name = args.next();

        let ctx = match discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return ReturnCode::Error,
        };

        if let Some(channel_name) = channel_name {
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
                return ReturnCode::OkEat;
            }
        } else if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
            let guild = guild.read();
            let guild_id = guild.id;
            drop(guild);

            let channels = utils::flatten_guilds(&ctx, &[GuildOrChannel::Guild(guild_id)]);

            buffers::create_buffers_from_flat_items(&ctx, &ctx.cache.read().user, &channels);
            return ReturnCode::OkEat;
        }
        if verbose {
            plugin_print("Couldn't find channel");
            return ReturnCode::OkEat;
        }
        ReturnCode::Error
    }
}

fn resolve_channel_id(guild_name: &str, channel_name: Option<&str>) -> Option<String> {
    let ctx = match discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return None,
    };

    if let Some(channel_name) = channel_name {
        if let Some((guild, channel)) =
            crate::utils::search_channel(&ctx.cache, guild_name, channel_name)
        {
            Some(crate::utils::unique_id(
                Some(guild.read().id),
                channel.read().id,
            ))
        } else {
            plugin_print("Unable to find server and channel");
            None
        }
    } else if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
        Some(crate::utils::unique_guild_id(guild.read().id))
    } else {
        plugin_print("Unable to find server");
        None
    }
}

fn add_item(items: Cow<str>, new_item: String) -> String {
    let mut items: Vec<_> = items.split(',').filter(|i| !i.is_empty()).collect();
    items.push(&new_item);
    items.sort_unstable();
    items.dedup();
    return items.join(",");
}

fn remove_item(items: Cow<str>, old_item: String) -> String {
    let items: Vec<_> = items
        .split(',')
        .filter(|i| !i.is_empty() && i != &old_item.as_str())
        .collect();
    return items.join(",");
}

fn watch(weechat: &Weechat, args: &Args) {
    if args.args.is_empty() {
        plugin_print("watch requires a guild name and optional channel name");
        return;
    }
    let mut args = args.args.iter().filter(|i| !i.is_empty());
    let guild_name = match args.next() {
        Some(g) => g,
        None => return,
    };
    let channel_name = args.next();

    let new_channel_id = match resolve_channel_id(guild_name, channel_name.cloned()) {
        Some(cid) => cid,
        None => return,
    };

    let weecord = crate::upgrade_plugin(weechat);
    let new_watched = add_item(weecord.config.watched_channels.value(), new_channel_id);
    let () = on_main_blocking(|weecord| {
        weecord.config.watched_channels.set(&new_watched);
    });
    if let Some(channel_name) = channel_name {
        plugin_print(&format!("Now watching {} in {}", guild_name, channel_name))
    } else {
        plugin_print(&format!("Now watching all of {}", guild_name))
    }
}

fn nowatch(weechat: &Weechat, args: &Args) {
    if args.args.is_empty() {
        plugin_print("nowatch requires a guild name and optional channel name");
        return;
    }
    let mut args = args.args.iter().filter(|i| !i.is_empty());
    let guild_name = match args.next() {
        Some(g) => g,
        None => return,
    };
    let channel_name = args.next();

    let new_channel_id = match resolve_channel_id(guild_name, channel_name.cloned()) {
        Some(cid) => cid,
        None => return,
    };

    let weecord = crate::upgrade_plugin(weechat);
    let new_watched = remove_item(weecord.config.watched_channels.value(), new_channel_id);
    let () = on_main_blocking(|weecord| {
        weecord.config.watched_channels.set(&new_watched);
    });
    if let Some(channel_name) = channel_name {
        plugin_print(&format!(
            "No longer watching {} in {}",
            guild_name, channel_name
        ))
    } else {
        plugin_print(&format!("No longer watching all of {}", guild_name))
    }
}

fn watched(weechat: &Weechat) {
    let mut channels = Vec::new();
    let mut guilds = Vec::new();

    let ctx = match discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    for watched_item in crate::upgrade_plugin(weechat).config.watched_channels() {
        match watched_item {
            utils::GuildOrChannel::Guild(guild) => guilds.push(guild),
            utils::GuildOrChannel::Channel(guild, channel) => channels.push((guild, channel)),
        }
    }

    if guilds.is_empty() && channels.is_empty() {
        weechat.print("There are no watched guilds or channels");
        return;
    }

    weechat.print("");

    weechat.print(&format!("Watched Servers: ({})", guilds.len()));
    for guild in guilds {
        if let Some(guild) = guild.to_guild_cached(ctx) {
            weechat.print(&format!("  {}", guild.read().name));
        }
    }

    weechat.print(&format!("Watched Channels: ({})", channels.len()));
    for (guild, channel) in channels {
        if let Ok(channel) = channel.to_channel(ctx) {
            let channel_name = channel.name();
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

fn autojoin(weechat: &Weechat, args: &Args, buffer: &Buffer) {
    if args.args.is_empty() {
        plugin_print("autojoin requires a guild name and optional channel name");
        return;
    }
    let mut opts = args.args.iter().filter(|i| !i.is_empty());
    let guild_name = match opts.next() {
        Some(g) => g,
        None => return,
    };
    let channel_name = opts.next();

    let new_channel_id = match resolve_channel_id(guild_name, channel_name.cloned()) {
        Some(cid) => cid,
        None => return,
    };

    let weecord = crate::upgrade_plugin(weechat);
    let new_autojoined = add_item(weecord.config.autojoin_channels.value(), new_channel_id);
    weecord.config.autojoin_channels.set(&new_autojoined);

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

fn noautojoin(weechat: &Weechat, args: &Args) {
    if args.args.is_empty() {
        plugin_print("noautojoin requires a guild name and optional channel name");
        return;
    }
    let mut opts = args.args.iter().filter(|i| !i.is_empty());
    let guild_name = match opts.next() {
        Some(g) => g,
        None => return,
    };
    let channel_name = opts.next();

    let channel_id = match resolve_channel_id(guild_name, channel_name.cloned()) {
        Some(cid) => cid,
        None => return,
    };

    let weecord = crate::upgrade_plugin(weechat);
    let new_autojoined = remove_item(weecord.config.autojoin_channels.value(), channel_id);
    weecord.config.autojoin_channels.set(&new_autojoined);

    if let Some(channel_name) = channel_name {
        plugin_print(&format!(
            "No longer autojoining {} in {}",
            guild_name, channel_name
        ));
    } else {
        plugin_print(&format!(
            "No longer autojoining all channels in {}",
            guild_name
        ))
    }
}

fn autojoined(weechat: &Weechat) {
    let mut channels = Vec::new();
    let mut guilds = Vec::new();

    let ctx = match discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    for autojoined_item in crate::upgrade_plugin(weechat).config.autojoin_channels() {
        match autojoined_item {
            utils::GuildOrChannel::Guild(guild) => guilds.push(guild),
            utils::GuildOrChannel::Channel(guild, channel) => channels.push((guild, channel)),
        }
    }

    if guilds.is_empty() && channels.is_empty() {
        weechat.print("There are no guilds or channels set to autojoin");
        return;
    }

    weechat.print("");

    weechat.print(&format!("Autojoin Servers: ({})", guilds.len()));
    for guild in guilds {
        if let Some(guild) = guild.to_guild_cached(ctx) {
            weechat.print(&format!("  {}", guild.read().name));
        }
    }

    weechat.print(&format!("Autojoin Channels: ({})", channels.len()));
    for (guild, channel) in channels {
        if let Ok(channel) = channel.to_channel(ctx) {
            let channel_name = channel.name();
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

fn status(args: &Args) {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let status_str = if args.args.is_empty() {
        "online"
    } else {
        args.args.get(0).unwrap()
    };

    let status = match status_str.to_lowercase().as_str() {
        "online" => OnlineStatus::Online,
        "offline" | "invisible" => OnlineStatus::Invisible,
        "idle" => OnlineStatus::Idle,
        "dnd" => OnlineStatus::DoNotDisturb,
        _ => {
            plugin_print(&format!("Unknown status \"{}\"", status_str));
            return;
        },
    };
    ctx.set_presence(None, status);
    *LAST_STATUS.lock() = status;
    plugin_print(&format!("Status set to {} {:#?}", status_str, status));
}

fn pins(weechat: &Discord, buffer: &Buffer) {
    let channel = buffer.channel_id();

    let channel_id = match channel {
        Some(ch) => ch,
        None => return,
    };

    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    let channel = match channel_id.to_channel_cached(ctx) {
        Some(ch) => ch,
        None => return,
    };

    buffers::create_pins_buffer(weechat, &channel);
    buffers::load_pin_buffer_history_for_id(channel.id());
}

fn game(args: &Args) {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };

    let activity = if args.args.is_empty() {
        None
    } else if args.args.len() == 1 {
        Some(Activity::playing(args.args.get(0).unwrap()))
    } else {
        let activity_type = args.args.get(0).unwrap();
        let activity = &args.rest[activity_type.len() + 1..];

        Some(match *activity_type {
            "playing" | "play" => Activity::playing(activity),
            "listening" => Activity::listening(activity),
            "watching" | "watch" => Activity::watching(activity),
            _ => {
                plugin_print(&format!("Unknown activity type \"{}\"", activity_type));
                return;
            },
        })
    };

    ctx.set_presence(activity, *LAST_STATUS.lock());
}

fn upload(args: &Args, buffer: &Buffer) {
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
            },
        };
        let full = full.as_str();
        // TODO: Check perms and file size
        let channel = if let Some(channel) = buffer.channel_id() {
            channel
        } else {
            return;
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
            },
        };
    }
}

// rust-lang/rust#52662 would let this api be improved by accepting option types
fn format_option_change<T: std::fmt::Display>(
    name: &str,
    value: &str,
    before: Option<&T>,
    change: weechat::OptionChanged,
) {
    use weechat::OptionChanged::*;
    let msg = match (change, before) {
        (Changed, Some(before)) => format!(
            "option {} successfully changed from {} to {}",
            name, before, value
        ),
        (Changed, None) | (Unchanged, None) => {
            format!("option {} successfully set to {}", name, value)
        },
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

fn discord_fmt(cmd: &str, msg: &str, buffer: &Buffer) {
    let msg = match cmd {
        "me" => format!("_{}_", msg),
        "tableflip" => format!("{} (╯°□°）╯︵ ┻━┻", msg),
        "unflip" => format!("{} ┬─┬ ノ( ゜-゜ノ)", msg),
        "shrug" => format!("{} ¯\\_(ツ)_/¯", msg),
        "spoiler" => format!("||{}||", msg),
        _ => unreachable!(),
    };

    let channel = if let Some(channel) = buffer.channel_id() {
        channel
    } else {
        return;
    };

    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let _ = channel.send_message(&ctx.http, |m| m.content(msg));
}

fn rehistory(weecord: &Discord, args: &Args, buffer: &MessageManager) {
    buffer.clear();
    let default_fetch_count = weecord.config.message_fetch_count.value();
    let count = args
        .args
        .front()
        .and_then(|c| c.parse::<i32>().ok())
        .unwrap_or(default_fetch_count);
    buffers::load_history(buffer, crossbeam_channel::unbounded().0, count);
}

const CMD_DESCRIPTION: weechat::CommandDescription = weechat::CommandDescription {
    name: "discord",
    description: "\
Discord from the comfort of your favorite command-line IRC client!
Source code available at https://github.com/terminal-discord/weechat-discord
Originally by https://github.com/khyperia/weechat-discord",
    args: "
    connect
    disconnect
    join
    query
    watch
    autojoin
    watched
    autojoined
    pins
    irc-mode
    discord-mode
    autostart
    noautostart
    token <token>
    upload <file>
    me
    tableflip
    unflip
    shrug
    spoiler
    rehistory",
    args_description: "
    connect: sign in to discord and open chat buffers
    disconnect: sign out of Discord
    join: join a channel in irc mode by providing guild name and channel name
    query: open a dm with a user (for when there are no discord buffers open)
    irc-mode: enable irc-mode, meaning that weecord will not load all channels like the official client
    discord-mode: enable discord-mode, meaning all available channels and guilds will be added to the buflist
    watch: Automatically open a buffer when a message is received in a guild or channel
    autojoin: Automatically open a channel or entire guild when discord connects
    watched: List watched guilds and channels
    autojoined: List autojoined guilds and channels
    pins: Show a list of pinned messages for the current channel
    autostart: automatically sign into discord on start
    noautostart: disable autostart
    status: set your Discord online status
    token: set Discord login token
    rehistory: reload the history in the current buffer
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
nowatch %(weecord_guild_completion) %(weecord_channel_completion) || \
watched || \
autojoined || \
autojoin %(weecord_guild_completion) %(weecord_channel_completion) || \
noautojoin %(weecord_guild_completion) %(weecord_channel_completion) || \
irc-mode || \
discord-mode || \
pins || \
token || \
autostart || \
noautostart || \
status online|offline|invisible|idle|dnd || \
game playing|listening|watching || \
upload %(filename) || \
me || \
tableflip || \
unflip || \
shrug || \
spoiler || \
rehistory || \
join %(weecord_guild_completion) %(weecord_channel_completion)",
};
