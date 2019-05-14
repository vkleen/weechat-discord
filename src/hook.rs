use crate::{
    buffers, discord,
    discord::DISCORD,
    ffi::{self, *},
    plugin_print,
};
use dirs;
use serenity::model::prelude::User;
use serenity::{
    model::{
        channel::Channel,
        id::{ChannelId, GuildId},
    },
    prelude::RwLock,
};
use std::{fs, ptr, sync::Arc, thread, time::Duration};

// *DO NOT* touch this outside of init/end
static mut MAIN_COMMAND_HOOK: *mut HookCommand = ptr::null_mut();
static mut BUFFER_SWITCH_CB: *mut SignalHook = ptr::null_mut();
static mut QUERY_CMD_HOOK: *mut HookCommandRun = ptr::null_mut();
static mut NICK_CMD_HOOK: *mut HookCommandRun = ptr::null_mut();
static mut TIMER_HOOK: *mut TimerHook = ptr::null_mut();
static mut GUILD_COMPLETION_HOOK: *mut Hook = ptr::null_mut();
static mut CHANNEL_COMPLETION_HOOK: *mut Hook = ptr::null_mut();
static mut DM_COMPLETION_HOOK: *mut Hook = ptr::null_mut();

pub fn init() -> Option<()> {
    let main_cmd_hook = ffi::hook_command(
        weechat_cmd::COMMAND,
        weechat_cmd::DESCRIPTION,
        weechat_cmd::ARGS,
        weechat_cmd::ARGDESC,
        weechat_cmd::COMPLETIONS,
        move |buffer, input| run_command(&buffer, input),
    )?;

    unsafe {
        crate::synchronization::MAIN_THREAD_ID = Some(std::thread::current().id());
    }

    let query_hook = ffi::hook_command_run("/query", handle_query)?;
    let nick_hook = ffi::hook_command_run("/nick", handle_nick)?;
    let buffer_switch_hook = ffi::hook_signal("buffer_switch", handle_buffer_switch)?;
    // TODO: Dynamic timer delay like weeslack?
    let timer_hook = ffi::hook_timer(50, 0, 0, handle_timer)?;

    let guild_completion_hook = ffi::hook_completion(
        "weecord_guild_completion",
        "Completion for discord guilds",
        handle_guild_completion,
    )?;

    let channel_completion_hook = ffi::hook_completion(
        "weecord_channel_completion",
        "Completion for discord channels",
        handle_channel_completion,
    )?;

    let dm_completion_hook = ffi::hook_completion(
        "weecord_dm_completion",
        "Completion for Discord private channels",
        handle_dm_completion,
    )?;

    unsafe {
        MAIN_COMMAND_HOOK = Box::into_raw(Box::new(main_cmd_hook));
        BUFFER_SWITCH_CB = Box::into_raw(Box::new(buffer_switch_hook));
        TIMER_HOOK = Box::into_raw(Box::new(timer_hook));
        QUERY_CMD_HOOK = Box::into_raw(Box::new(query_hook));
        NICK_CMD_HOOK = Box::into_raw(Box::new(nick_hook));
        GUILD_COMPLETION_HOOK = Box::into_raw(Box::new(guild_completion_hook));
        CHANNEL_COMPLETION_HOOK = Box::into_raw(Box::new(channel_completion_hook));
        DM_COMPLETION_HOOK = Box::into_raw(Box::new(dm_completion_hook))
    };
    Some(())
}

pub fn destroy() {
    unsafe {
        let _ = Box::from_raw(MAIN_COMMAND_HOOK);
        MAIN_COMMAND_HOOK = ptr::null_mut();
        let _ = Box::from_raw(BUFFER_SWITCH_CB);
        BUFFER_SWITCH_CB = ptr::null_mut();
        let _ = Box::from_raw(TIMER_HOOK);
        TIMER_HOOK = ptr::null_mut();
        let _ = Box::from_raw(QUERY_CMD_HOOK);
        QUERY_CMD_HOOK = ptr::null_mut();
        let _ = Box::from_raw(NICK_CMD_HOOK);
        NICK_CMD_HOOK = ptr::null_mut();
        let _ = Box::from_raw(CHANNEL_COMPLETION_HOOK);
        CHANNEL_COMPLETION_HOOK = ptr::null_mut();
    };
}

#[allow(clippy::needless_pass_by_value)]
fn handle_buffer_switch(data: SignalHookData) {
    if let SignalHookData::Pointer(buffer) = data {
        thread::spawn(move || {
            buffers::load_history(&buffer);
            buffers::load_nicks(&buffer);
        });
    }
}

fn handle_timer(_remaining: i32) {
    while let Ok(_) = crate::synchronization::WEE_SYNC.try_recv() {
        let _ = crate::synchronization::WEE_SYNC.send();
    }
}

// TODO: Transform irc/weechat style to discord style
#[allow(clippy::needless_pass_by_value)]
pub fn buffer_input(buffer: Buffer, message: &str) {
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
        let http = &ctx.http;
        channel
            .say(http, message)
            .unwrap_or_else(|_| panic!("Unable to send message to {}", channel.0));
    }
}

fn handle_channel_completion(
    buffer: Buffer,
    _completion_item: &str,
    mut completion: ffi::Completion,
) {
    // Get the previous argument with should be the guild name
    // TODO: Generalize this?
    let input = buffer.get("input").and_then(|i| {
        let x = i.split(' ').collect::<Vec<_>>();
        if x.len() < 2 {
            None
        } else {
            Some(x[x.len() - 2].to_owned())
        }
    });
    let input = match input {
        Some(i) => i,
        None => return,
    };

    // Match mangled name to the real name
    let ctx = match discord::get_ctx() {
        Some(s) => s,
        None => return,
    };

    for guild in ctx.cache.read().guilds.values() {
        let guild = guild.read();
        if parsing::weechat_arg_strip(&guild.name) == input {
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
            return;
        }
    }
}

fn handle_guild_completion(
    _buffer: Buffer,
    _completion_item: &str,
    mut completion: ffi::Completion,
) {
    let ctx = match discord::get_ctx() {
        Some(s) => s,
        None => return,
    };
    for guild in ctx.cache.read().guilds.values() {
        let name = parsing::weechat_arg_strip(&guild.read().name);
        completion.add(&name);
    }
}

fn handle_dm_completion(_buffer: Buffer, _completion_time: &str, mut completion: ffi::Completion) {
    let ctx = match discord::get_ctx() {
        Some(s) => s,
        None => return,
    };
    for dm in ctx.cache.read().private_channels.values() {
        completion.add(&dm.read().recipient.read().name);
    }
}

// TODO: Make this faster
// TODO: Handle command options
fn handle_query(_buffer: Buffer, command: &str) -> i32 {
    let owned_cmd = command.to_owned();
    thread::spawn(move || {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };
        let http = &ctx.http;
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
            let guilds = current_user
                .guilds(&ctx.http)
                .expect("Unable to fetch guilds");
            for guild in &guilds {
                if let Some(guild) = guild.id.to_guild_cached(&ctx.cache) {
                    let guild = guild.read().clone();
                    for m in guild.members_containing(substr, false, true) {
                        found_members.push(m.user.read().clone());
                    }
                }
            }
        }
        found_members.dedup_by_key(|mem| mem.id);

        if let Some(target) = found_members.get(0) {
            if let Ok(chan) = target.create_dm_channel(http) {
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
    1
}

// TODO: Handle command options
fn handle_nick(buffer: Buffer, command: &str) -> i32 {
    let guilds;
    let mut substr;
    {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return 2,
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
                .guilds(&ctx.http)
                .unwrap_or_default()
                .iter()
                .map(|g| g.id)
                .collect()
        } else {
            let guild = on_main! {{
                let guild = match buffer.get("localvar_guildid") {
                    Some(guild) => guild,
                    None => return 1,
                };
                match guild.parse::<u64>() {
                    Ok(v) => GuildId(v),
                    Err(_) => return 1,
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
            let http = &ctx.http;
            for guild in guilds {
                let new_nick = if substr.is_empty() {
                    None
                } else {
                    Some(substr.as_str())
                };
                let _ = guild.edit_nickname(&http, new_nick);
                // Make it less spammy
                thread::sleep(Duration::from_secs(1));
            }
        }

        on_main! {{
            buffers::update_nick();
        }};
    });
    1
}

fn run_command(_buffer: &Buffer, command: &str) {
    // TODO: Add rename command
    // TODO: Get a proper parser
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
            handle_query(
                Buffer::current().expect("there should always be a buffer"),
                &format!("/{}", command),
            );
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
                    return;
                }
            } else {
                if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
                    crate::utils::unique_guild_id(guild.read().id)
                } else {
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
        }
        "watch" => {
            plugin_print("watch requires a guild name and channel name");
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
                    return;
                }
            } else {
                if let Some(guild) = crate::utils::search_guild(&ctx.cache, guild_name) {
                    crate::utils::unique_guild_id(guild.read().id)
                } else {
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
        }
        "autojoin" => {
            plugin_print("autojoin requires a guild name and channel name");
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
            let buffer = match Buffer::current() {
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
            let http = &ctx.http;
            match channel.send_files(http, vec![full], |m| m) {
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
         autojoin %(weecord_guild_completion) %(weecord_channel_completion) || \
         irc-mode || \
         discord-mode || \
         token || \
         autostart || \
         noautostart || \
         upload %(filename) || \
         join %(weecord_guild_completion) %(weecord_channel_completion)";
}
