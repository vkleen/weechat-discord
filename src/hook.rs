use ffi::{self, *};
use std::ptr;
use std::sync::Arc;
use std::thread;
use {buffers, discord, discord::DISCORD, plugin_print};

use serenity::model::channel::Channel;
use serenity::model::id::{ChannelId, GuildId};
use serenity::prelude::RwLock;
use serenity::CACHE;

// *DO NOT* touch this outside of init/end
static mut MAIN_COMMAND_HOOK: *mut HookCommand = ptr::null_mut();
static mut BUFFER_SWITCH_CB: *mut SignalHook = ptr::null_mut();
static mut TRIGGER_CB: *mut SignalHook = ptr::null_mut();
static mut QUERY_CMD_HOOK: *mut HookCommandRun = ptr::null_mut();
static mut NICK_CMD_HOOK: *mut HookCommandRun = ptr::null_mut();

pub fn init() -> Option<()> {
    let main_cmd_hook = ffi::hook_command(
        weechat_cmd::COMMAND,
        weechat_cmd::DESCRIPTION,
        weechat_cmd::ARGS,
        weechat_cmd::ARGDESC,
        weechat_cmd::COMPLETIONS,
        move |buffer, input| run_command(&buffer, input),
    )?;

    let query_hook = ffi::hook_command_run("/query", handle_query)?;
    let nick_hook = ffi::hook_command_run("/nick", handle_nick)?;
    let buffer_switch_hook = ffi::hook_signal("buffer_switch", handle_buffer_switch)?;
    let trigger_hook = ffi::hook_signal("main_thread_lock", handle_trigger)?;

    unsafe {
        MAIN_COMMAND_HOOK = Box::into_raw(Box::new(main_cmd_hook));
        BUFFER_SWITCH_CB = Box::into_raw(Box::new(buffer_switch_hook));
        TRIGGER_CB = Box::into_raw(Box::new(trigger_hook));
        QUERY_CMD_HOOK = Box::into_raw(Box::new(query_hook));
        NICK_CMD_HOOK = Box::into_raw(Box::new(nick_hook));
    };
    Some(())
}

pub fn destroy() {
    unsafe {
        let _ = Box::from_raw(MAIN_COMMAND_HOOK);
        MAIN_COMMAND_HOOK = ptr::null_mut();
        let _ = Box::from_raw(BUFFER_SWITCH_CB);
        BUFFER_SWITCH_CB = ptr::null_mut();
        let _ = Box::from_raw(TRIGGER_CB);
        TRIGGER_CB = ptr::null_mut();
        let _ = Box::from_raw(QUERY_CMD_HOOK);
        QUERY_CMD_HOOK = ptr::null_mut();
        let _ = Box::from_raw(NICK_CMD_HOOK);
        NICK_CMD_HOOK = ptr::null_mut();
    };
}

#[allow(needless_pass_by_value)]
fn handle_buffer_switch(data: SignalHookData) {
    if let SignalHookData::Pointer(buffer) = data {
        thread::spawn(move || {
            buffers::load_history(&buffer);
            buffers::load_nicks(&buffer);
        });
    }
}

#[allow(needless_pass_by_value)]
fn handle_trigger(_data: SignalHookData) {
    if let Ok(tx) = ::synchronization::TX.lock() {
        if let Some(ref tx) = *tx {
            tx.send(()).unwrap();
        }
    }

    if let Ok(tx) = ::synchronization::TX.lock() {
        if let Some(ref tx) = *tx {
            tx.send(()).unwrap();
        }
    }
}

// TODO: Transform irc/weechat style to discord style
#[allow(needless_pass_by_value)]
pub fn buffer_input(buffer: Buffer, message: &str) {
    let channel = buffer
        .get("localvar_channelid")
        .and_then(|id| id.parse().ok())
        .map(ChannelId);

    let message = ffi::remove_color(message);

    if let Some(channel) = channel {
        channel
            .say(message)
            .unwrap_or_else(|_| panic!("Unable to send message to {}", channel.0));
    }
}

// TODO: Make this faster
// TODO: Handle command options
fn handle_query(_buffer: Buffer, command: &str) {
    let current_user = &CACHE.read().user;
    let substr = &command["/query ".len()..].trim();

    let mut found_members = Vec::new();
    for guild in current_user.guilds().expect("Unable to fetch guilds") {
        if let Some(guild) = guild.id.to_guild_cached() {
            let guild = guild.read().clone();
            for m in guild.members_containing(substr, false, true) {
                found_members.push(m.clone());
            }
        }
    }
    found_members.dedup_by_key(|mem| mem.user.read().id);
    if let Some(target) = found_members.get(0) {
        if let Ok(chan) = target.user.read().create_dm_channel() {
            buffers::create_buffer_from_dm(
                Channel::Private(Arc::new(RwLock::new(chan))),
                &current_user.name,
                true,
            );
        }
    }
}

// TODO: Handle command options
fn handle_nick(buffer: Buffer, command: &str) {
    let substr = command["/nick".len()..].trim().to_owned();
    let guild = on_main! {{
        let guild = match buffer.get("localvar_guildid") {
            Some(guild) => guild,
            None => return,
        };
        match guild.parse::<u64>() {
            Ok(v) => GuildId(v),
            Err(_) => return,
        }
    }};
    thread::spawn(move || {
        let new_nick = if substr.is_empty() {
            None
        } else {
            Some(substr.as_str())
        };
        let _ = guild.edit_nickname(new_nick);
        buffers::update_nick();
    });
}

fn run_command(_buffer: &Buffer, command: &str) {
    // TODO: Add rename command
    match command {
        "" => plugin_print("see /help discord for more information"),
        "connect" => {
            match ffi::get_option("token") {
                Some(t) => {
                    if DISCORD.lock().is_none() {
                        discord::init(&t);
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
            }
            plugin_print("Disconnected");
        }
        _ if command.starts_with("token ") => {
            let token = &command["token ".len()..];
            user_set_option("token", token.trim_matches('"'));
            plugin_print("Set Discord token");
        }
        "autostart" => {
            set_option("autostart", "true");
            plugin_print("Discord will now load on startup");
        }
        "noautostart" => {
            set_option("autostart", "false");
            plugin_print("Discord will not load on startup");
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
Source code available at https://github.com/Noskcaj19/weechat-discord
Originally by https://github.com/khyperia/weechat-discord
Options used:
plugins.var.weecord.token = <discord_token>
plugins.var.weecord.rename.<id> = <string>
plugins.var.weecord.autostart = <bool>
";
    pub const ARGS: &str = "\
                     connect
                     disconnect
                     autostart
                     noautostart
                     token <token>";
    pub const ARGDESC: &'static str = "\
connect: sign in to discord and open chat buffers
disconnect: sign out of Discord
autostart: automatically sign into discord on start
noautostart: disable autostart
token: set Discord login token
Example:
  /discord token 123456789ABCDEF
  /discord connect
  /discord autostart
  /discord disconnect
";
    pub const COMPLETIONS: &str = "\
                                   connect || disconnect || token || autostart || noautostart";
}
