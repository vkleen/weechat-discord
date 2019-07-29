use crate::{discord, on_main, plugin_print};
use serenity::{model::prelude::*, prelude::*};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use weechat::{Buffer, ReturnCode};

pub struct HookHandles {
    _buffer_switch_handle: weechat::SignalHook<()>,
    _command_handle: weechat::CommandHook<()>,
    _query_handle: weechat::CommandRunHook<()>,
    _nick_handle: weechat::CommandRunHook<()>,
    _guild_completion_handle: weechat::CompletionHook<()>,
    _channel_completion_handle: weechat::CompletionHook<()>,
    _dm_completion_handle: weechat::CompletionHook<()>,
}

pub fn init(weechat: &weechat::Weechat) -> HookHandles {
    let _command_handle = crate::command::init(weechat);

    let _buffer_switch_handle = weechat.hook_signal(
        "buffer_switch",
        |_, value| handle_buffer_switch(value),
        None,
    );

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

    HookHandles {
        _buffer_switch_handle,
        _command_handle,
        _query_handle,
        _nick_handle,
        _guild_completion_handle,
        _channel_completion_handle,
        _dm_completion_handle,
    }
}

pub fn buffer_input(buffer: Buffer, text: &str) {
    let channel = buffer
        .get_localvar("channelid")
        .and_then(|id| id.parse().ok())
        .map(ChannelId);

    if let Some(channel) = channel {
        let ctx = match crate::discord::get_ctx() {
            Some(ctx) => ctx,
            _ => return,
        };
        channel
            .say(ctx, text)
            .unwrap_or_else(|_| panic!("Unable to send message to {}", channel.0));
    }
}

fn handle_buffer_switch(data: weechat::SignalHookValue) -> ReturnCode {
    if let weechat::SignalHookValue::Pointer(buffer_ptr) = data {
        let buffer = unsafe { crate::utils::buffer_from_ptr(buffer_ptr) };

        if buffer.get_localvar("loaded_history").is_none() {
            crate::buffers::load_history(&buffer);
        }

        if buffer.get_localvar("loaded_nicks").is_none() {
            crate::buffers::load_nicks(&buffer);
        }
    }
    ReturnCode::Ok
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
pub(crate) fn handle_query(buffer: &Buffer, command: &str) -> ReturnCode {
    if command.len() <= "/query ".len() {
        plugin_print("query requires a username");
        return ReturnCode::Ok;
    }

    if buffer.get_localvar("guildid").is_none() {
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
        for private_channel in ctx.cache.read().private_channels.values() {
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

        let current_user_name = current_user.name.clone();

        if let Some(target) = found_members.get(0) {
            if let Ok(chan) = target.create_dm_channel(ctx) {
                on_main(move |weechat| {
                    crate::buffers::create_buffer_from_dm(
                        &weechat,
                        Channel::Private(Arc::new(RwLock::new(chan))),
                        &current_user_name,
                        true,
                    );
                });
                return;
            }
        }

        plugin_print(&format!("Could not find user {:?}", substr));
    });
    ReturnCode::OkEat
}

// TODO: Handle command options
fn handle_nick(buffer: &Buffer, command: &str) -> ReturnCode {
    if buffer.get_localvar("guildid").is_none() {
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
        let mut split = substr.split(' ');
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
            let guild = buffer
                .get_localvar("guildid")
                .expect("must to be some, checked at top of function");
            let guild = match guild.parse::<u64>() {
                Ok(v) => GuildId(v),
                Err(_) => return ReturnCode::OkEat,
            };
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

        crate::buffers::update_nick();
    });
    ReturnCode::OkEat
}
