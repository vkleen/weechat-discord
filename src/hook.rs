use crate::{
    buffers, discord,
    discord::DISCORD,
    ffi::{self, set_option, MAIN_BUFFER},
    plugin_print, utils,
};

use weechat::{Buffer, ReturnCode};

use dirs;
use serenity::{
    model::prelude::User,
    model::{
        channel::Channel,
        id::{ChannelId, GuildId},
    },
    prelude::RwLock,
};
use std::{fs, sync::Arc, thread, time::Duration};

pub struct HookHandles {
    _cmd_handle: weechat::CommandHook<()>,
    _timer_handle: weechat::TimerHook<()>,
    _query_handle: weechat::CommandRunHook<()>,
    _nick_handle: weechat::CommandRunHook<()>,
    _buffer_switch_handle: weechat::SignalHook<()>,
    _guild_completion_handle: weechat::CompletionHook<()>,
    _channel_completion_handle: weechat::CompletionHook<()>,
    _dm_completion_handle: weechat::CompletionHook<()>,
}

pub fn init(weechat: &weechat::Weechat) -> Option<HookHandles> {
    unsafe {
        crate::synchronization::MAIN_THREAD_ID = Some(std::thread::current().id());
    }

    let cmd_description = weechat::CommandDescription {
        name: weechat_cmd::COMMAND,
        description: weechat_cmd::DESCRIPTION,
        args: weechat_cmd::ARGS,
        args_description: weechat_cmd::ARGDESC,
        completion: weechat_cmd::COMPLETIONS,
    };

    let _cmd_handle = weechat.hook_command(
        cmd_description,
        |_, buffer, args| run_command(&buffer, &args.collect::<Vec<_>>().join(" ")),
        None,
    );

    // TODO: Dynamic timer delay like weeslack?
    let _timer_handle = weechat.hook_timer(50, 0, 0, |_, remaining| handle_timer(remaining), None);

    let _query_handle = weechat.hook_command_run(
        "/query",
        |_, ref buffer, command| handle_query(buffer, command),
        None,
    );

    let _nick_handle = weechat.hook_command_run(
        "/nick",
        |_, ref buffer, command| handle_nick(buffer, command),
        None,
    );

    let _buffer_switch_handle = weechat.hook_signal(
        "buffer_switch",
        |_, value| handle_buffer_switch(value),
        None,
    );

    let _guild_completion_handle = weechat.hook_completion(
        "weecord_guild_completion",
        "Completion for discord guilds",
        |_, ref buffer, item, completions| handle_guild_completion(buffer, item, completions),
        None,
    );

    let _channel_completion_handle = weechat.hook_completion(
        "weecord_channel_completion",
        "Completion for discord channels",
        |_, ref buffer, item, completions| handle_channel_completion(buffer, item, completions),
        None,
    );

    let _dm_completion_handle = weechat.hook_completion(
        "weecord_dm_completion",
        "Completion for Discord private channels",
        |_, ref buffer, item, completions| handle_dm_completion(buffer, item, completions),
        None,
    );

    Some(HookHandles {
        _cmd_handle,
        _timer_handle,
        _query_handle,
        _nick_handle,
        _buffer_switch_handle,
        _guild_completion_handle,
        _channel_completion_handle,
        _dm_completion_handle,
    })
}

#[allow(clippy::needless_pass_by_value)]
fn handle_buffer_switch(data: weechat::SignalHookValue) -> ReturnCode {
    if let weechat::SignalHookValue::Pointer(buffer_ptr) = data {
        let buffer = ffi::Buffer::from_ptr(buffer_ptr);
        thread::spawn(move || {
            buffers::load_history(&buffer);
            buffers::load_nicks(&buffer);
        });
    }
    ReturnCode::Ok
}

fn handle_timer(_remaining: i32) {
    while let Ok(_) = crate::synchronization::WEE_SYNC.try_recv() {
        let _ = crate::synchronization::WEE_SYNC.send();
    }
}

// TODO: Transform irc/weechat style to discord style
#[allow(clippy::needless_pass_by_value)]
pub fn buffer_input(buffer: ffi::Buffer, message: &str) {
    let channel = buffer
        .get("localvar_channelid")
        .and_then(|id| id.parse().ok())
        .map(ChannelId);

    let message = ffi::remove_color(message);

    if let Some(channel) = channel {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };
        channel
            .say(ctx, message)
            .unwrap_or_else(|_| panic!("Unable to send message to {}", channel.0));
    }
}

fn handle_channel_completion(
    buffer: &Buffer,
    _completion_item: &str,
    completion: weechat::Completion,
) -> ReturnCode {
    // Get the previous argument with should be the guild name
    // TODO: Generalize this?
    let x = buffer.input().split(' ').collect::<Vec<_>>();
    let input = if x.len() < 2 {
        None
    } else {
        Some(x[x.len() - 2].to_owned())
    };

    let input = match input {
        Some(i) => i,
        None => return ReturnCode::Ok,
    };

    // Match mangled name to the real name
    let ctx = match discord::get_ctx() {
        Some(s) => s,
        None => return ReturnCode::Ok,
    };

    for guild in ctx.cache.read().guilds.values() {
        let guild = guild.read();
        if parsing::weechat_arg_strip(&guild.name).to_lowercase() == input.to_lowercase() {
            for channel in guild.channels.values() {
                let channel = channel.read();
                // Skip non text channels
                use serenity::model::channel::ChannelType::*;
                match channel.kind {
                    Text | Private | Group | News => {}
                    _ => continue,
                }
                completion.add(&parsing::weechat_arg_strip(&channel.name))
            }
            return ReturnCode::Ok;
        }
    }
    ReturnCode::Ok
}

fn handle_guild_completion(
    _buffer: &Buffer,
    _completion_item: &str,
    completion: weechat::Completion,
) -> ReturnCode {
    let ctx = match discord::get_ctx() {
        Some(s) => s,
        None => return ReturnCode::Ok,
    };
    for guild in ctx.cache.read().guilds.values() {
        let name = parsing::weechat_arg_strip(&guild.read().name);
        completion.add(&name);
    }
    ReturnCode::Ok
}

fn handle_dm_completion(
    _buffer: &Buffer,
    _completion_time: &str,
    completion: weechat::Completion,
) -> ReturnCode {
    let ctx = match discord::get_ctx() {
        Some(s) => s,
        None => return ReturnCode::Ok,
    };
    for dm in ctx.cache.read().private_channels.values() {
        completion.add(&dm.read().recipient.read().name);
    }
    ReturnCode::Ok
}

// TODO: Make this faster
// TODO: Handle command options
fn handle_query(buffer: &Buffer, command: &str) -> ReturnCode {
    if command.len() <= "/query ".len() {
        plugin_print("query requires a username");
        return ReturnCode::Ok;
    }

    if buffer.get_localvar("guildid").is_empty() {
        return ReturnCode::Ok;
    };

    let owned_cmd = command.to_owned();
    thread::spawn(move || {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };
        let current_user = &ctx.cache.read().user;
        let substr = &owned_cmd["/query ".len()..].trim();

        let mut found_members: Vec<User> = Vec::new();
        for (_, private_channel) in &ctx.cache.read().private_channels {
            if private_channel
                .read()
                .name()
                .to_lowercase()
                .contains(&substr.to_lowercase())
            {
                found_members.push(private_channel.read().recipient.read().clone())
            }
        }

        if found_members.is_empty() {
            let guilds = current_user.guilds(ctx).expect("Unable to fetch guilds");
            for guild in &guilds {
                if let Some(guild) = guild.id.to_guild_cached(ctx) {
                    let guild = guild.read().clone();
                    for m in guild.members_containing(substr, false, true) {
                        found_members.push(m.user.read().clone());
                    }
                }
            }
        }
        found_members.dedup_by_key(|mem| mem.id);

        if let Some(target) = found_members.get(0) {
            if let Ok(chan) = target.create_dm_channel(ctx) {
                buffers::create_buffer_from_dm(
                    Channel::Private(Arc::new(RwLock::new(chan))),
                    &current_user.name,
                    true,
                );
                return;
            }
        }
        plugin_print(&format!("Could not find user {:?}", substr));
    });
    ReturnCode::OkEat
}

// TODO: Handle command options
fn handle_nick(buffer: &Buffer, command: &str) -> ReturnCode {
    if buffer.get_localvar("guildid").is_empty() {
        return ReturnCode::Ok;
    };

    let guilds;
    let mut substr;
    {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return ReturnCode::Error,
        };
        substr = command["/nick".len()..].trim().to_owned();
        let mut split = substr.split(" ");
        let all = split.next() == Some("-all");
        if all {
            substr = substr["-all".len()..].trim().to_owned();
        }
        guilds = if all {
            let current_user = &ctx.cache.read().user;

            // TODO: Error handling
            current_user
                .guilds(ctx)
                .unwrap_or_default()
                .iter()
                .map(|g| g.id)
                .collect()
        } else {
            let guild = on_main! {{
                let guild = buffer.get_localvar("guildid");
                match guild.parse::<u64>() {
                    Ok(v) => GuildId(v),
                    Err(_) => return ReturnCode::OkEat,
                }
            }};
            vec![guild]
        };
    }

    thread::spawn(move || {
        {
            let ctx = match crate::discord::get_ctx() {
                Some(ctx) => ctx,
                _ => return,
            };
            for guild in guilds {
                let new_nick = if substr.is_empty() {
                    None
                } else {
                    Some(substr.as_str())
                };
                let _ = guild.edit_nickname(ctx, new_nick);
                // Make it less spammy
                thread::sleep(Duration::from_secs(1));
            }
        }

        on_main! {{
            buffers::update_nick();
        }};
    });
    ReturnCode::OkEat
}

fn run_command(buffer: &Buffer, command: &str) {
    // TODO: Add rename command
    // TODO: Get a proper parser
    let command = command["/discord".len()..].trim();

    match command {
        "" => plugin_print("see /help discord for more information"),
        "connect" => {
            match ffi::get_option("token") {
                Some(t) => {
                    if DISCORD.lock().is_none() {
                        discord::init(&t, crate::utils::get_irc_mode());
                    } else {
                        plugin_print("Already connected");
                    }
                }
                None => {
                    plugin_print("Error: plugins.var.weecord.token unset. Run:");
                    plugin_print("/discord token 123456789ABCDEF");
                    return;
                }
            };
        }
        "disconnect" => {
            let mut discord = DISCORD.lock();
            if discord.is_some() {
                if let Some(discord) = discord.take() {
                    discord.shutdown();
                };
                plugin_print("Disconnected");
            } else {
                plugin_print("Already disconnected");
            }
        }
        "irc-mode" => {
            if crate::utils::get_irc_mode() {
                plugin_print("irc-mode already enabled")
            } else {
                user_set_option("irc_mode", "true");
                plugin_print("irc-mode enabled")
            }
        }
        "discord-mode" => {
            if !crate::utils::get_irc_mode() {
                plugin_print("discord-mode already enabled")
            } else {
                user_set_option("irc_mode", "false");
                plugin_print("discord-mode enabled")
            }
        }
        _ if command.starts_with("token ") => {
            let token = &command["token ".len()..];
            user_set_option("token", token.trim_matches('"'));
            plugin_print("Set Discord token");
        }
        "token" => {
            plugin_print("token requires an argument");
        }
        "autostart" => {
            set_option("autostart", "true");
            plugin_print("Discord will now load on startup");
        }
        "noautostart" => {
            set_option("autostart", "false");
            plugin_print("Discord will not load on startup");
        }
        _ if command.starts_with("query ") => {
            handle_query(buffer, &format!("/{}", command));
        }
        "query" => {
            plugin_print("query requires a username");
        }
        _ if command.starts_with("join ") => {
            let mut args = command["join ".len()..].split(' ');
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
                    &channel.read(),
                    &ctx.cache.read().user.name,
                    false,
                );
                return;
            }
            plugin_print("Couldn't find channel")
        }
        "join" => {
            plugin_print("join requires an guild name and channel name");
        }
        _ if command.starts_with("watch ") => {
            let mut args = command["watch ".len()..]
                .split(' ')
                .filter(|i| !i.is_empty());
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
            } else {
                if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
                    crate::utils::unique_guild_id(guild.read().id)
                } else {
                    plugin_print("Unable to find server");
                    return;
                }
            };
            let new_watched = if let Some(watched_channels) = ffi::get_option("watched_channels") {
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
            ffi::set_option("watched_channels", &new_watched);
            if let Some(channel_name) = channel_name {
                plugin_print(&format!("Now watching {} in {}", guild_name, channel_name))
            } else {
                plugin_print(&format!("Now watching all of {}", guild_name))
            }
        }
        "watch" => {
            plugin_print("watch requires a guild name and channel name");
        }
        "watched" => {
            MAIN_BUFFER.print("");
            let mut channels = Vec::new();
            let mut guilds = Vec::new();

            let ctx = match discord::get_ctx() {
                Some(ctx) => ctx,
                _ => return,
            };

            match ffi::get_option("watched_channels") {
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

            MAIN_BUFFER.print(&format!("Watched Servers: ({})", guilds.len()));
            for guild in guilds {
                if let Some(guild) = guild.to_guild_cached(ctx) {
                    MAIN_BUFFER.print(&format!("  {}", guild.read().name));
                }
            }

            MAIN_BUFFER.print(&format!("Watched Channels: ({})", channels.len()));
            for (guild, channel) in channels {
                if let Ok(channel) = channel.to_channel(ctx) {
                    let channel_name = utils::channel_name(&channel);
                    if let Some(guild) = guild {
                        let guild_name = if let Some(guild) = guild.to_guild_cached(&ctx) {
                            guild.read().name.to_owned()
                        } else {
                            guild.0.to_string()
                        };
                        MAIN_BUFFER.print(&format!("  {}: {}", guild_name, channel_name));
                    } else {
                        MAIN_BUFFER.print(&format!("  {}", channel_name));
                    }
                } else {
                    MAIN_BUFFER.print(&format!("  {:?} {:?}", guild, channel));
                }
            }
        }
        _ if command.starts_with("autojoin ") => {
            let mut args = command["autojoin ".len()..]
                .split(' ')
                .filter(|i| !i.is_empty());
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
            } else {
                if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
                    crate::utils::unique_guild_id(guild.read().id)
                } else {
                    plugin_print("Unable to find server");
                    return;
                }
            };
            let new_autojoined =
                if let Some(autojoined_channels) = ffi::get_option("autojoin_channels") {
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
            ffi::set_option("autojoin_channels", &new_autojoined);

            if let Some(channel_name) = channel_name {
                plugin_print(&format!(
                    "Now autojoining {} in {}",
                    guild_name, channel_name
                ));
                plugin_print(&format!("join {}", &command["autojoin ".len()..]));
                run_command(buffer, &format!("join {}", &command["autojoin ".len()..]));
            } else {
                plugin_print(&format!("Now autojoining all channels in {}", guild_name))
            }
        }
        "autojoin" => {
            plugin_print("autojoin requires a guild name and channel name");
        }
        "autojoined" => {
            MAIN_BUFFER.print("");
            let mut channels = Vec::new();
            let mut guilds = Vec::new();

            let ctx = match discord::get_ctx() {
                Some(ctx) => ctx,
                _ => return,
            };

            match ffi::get_option("autojoin_channels") {
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

            MAIN_BUFFER.print(&format!("Autojoin Servers: ({})", guilds.len()));
            for guild in guilds {
                if let Some(guild) = guild.to_guild_cached(ctx) {
                    MAIN_BUFFER.print(&format!("  {}", guild.read().name));
                }
            }

            MAIN_BUFFER.print(&format!("Autojoin Channels: ({})", channels.len()));
            for (guild, channel) in channels {
                if let Ok(channel) = channel.to_channel(ctx) {
                    let channel_name = utils::channel_name(&channel);
                    if let Some(guild) = guild {
                        let guild_name = if let Some(guild) = guild.to_guild_cached(&ctx) {
                            guild.read().name.to_owned()
                        } else {
                            guild.0.to_string()
                        };
                        MAIN_BUFFER.print(&format!("  {}: {}", guild_name, channel_name));
                    } else {
                        MAIN_BUFFER.print(&format!("  {}", channel_name));
                    }
                } else {
                    MAIN_BUFFER.print(&format!("  {:?} {:?}", guild, channel));
                }
            }
        }
        _ if command.starts_with("upload ") => {
            let mut file = command["upload ".len()..].to_owned();
            // TODO: Find a better way to expand paths
            if file.starts_with("~/") {
                let rest: String = file.chars().skip(2).collect();
                let dir = match dirs::home_dir() {
                    Some(dir) => dir.to_string_lossy().into_owned(),
                    None => ".".to_owned(),
                };
                file = format!("{}/{}", dir, rest);
            }
            let full = match fs::canonicalize(file) {
                Ok(f) => f.to_string_lossy().into_owned(),
                Err(e) => {
                    plugin_print(&format!("Unable to resolve file path: {}", e));
                    return;
                }
            };
            let full = full.as_str();
            // TODO: Check perms and file size
            let buffer = match ffi::Buffer::current() {
                Some(buf) => buf,
                None => return,
            };
            let channel = match buffer.get("localvar_channelid") {
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
                Err(e) => match e {
                    serenity::Error::Model(serenity::model::ModelError::MessageTooLong(_)) => {
                        plugin_print("File too large to upload");
                    }
                    _ => {}
                },
            };
        }
        _ if command.contains("upload") => {
            plugin_print("upload requires an argument");
        }
        _ => {
            plugin_print("Unknown command");
        }
    };
}

fn user_set_option(name: &str, value: &str) {
    plugin_print(&ffi::set_option(name, value));
}

mod weechat_cmd {
    pub const COMMAND: &str = "discord";
    pub const DESCRIPTION: &str = "\
Discord from the comfort of your favorite command-line IRC client!
Source code available at https://github.com/terminal-discord/weechat-discord
Originally by https://github.com/khyperia/weechat-discord
Options used:
plugins.var.weecord.token = <discord_token>
plugins.var.weecord.rename.<id> = <string>
plugins.var.weecord.autostart = <bool>
plugins.var.weecord.irc_mode = <bool>
";
    pub const ARGS: &str = "\
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
                     upload <file>";
    pub const ARGDESC: &'static str = "\
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
autostart: automatically sign into discord on start
noautostart: disable autostart
token: set Discord login token
upload: upload a file to the current channel
Example:
  /discord token 123456789ABCDEF
  /discord connect
  /discord autostart
  /discord disconnect
  /discord upload file.txt
";
    pub const COMPLETIONS: &str =
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
         join %(weecord_guild_completion) %(weecord_channel_completion)";
}
