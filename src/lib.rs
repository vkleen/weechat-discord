#![allow(clippy::let_unit_value)]

mod bar_items;
mod buffers;
mod command;
mod config;
mod discord;
mod hook;
mod printing;
mod sync;
mod utils;

pub use sync::{on_main, on_main_blocking, upgrade_plugin};

use std::borrow::Cow;
use weechat::{weechat_plugin, ArgsWeechat, ConfigOption, Weechat, WeechatPlugin, WeechatResult};

pub struct Discord {
    weechat: Weechat,
    config: config::Config,
    _sync_handle: sync::SyncHandle,
    _hook_handles: hook::HookHandles,
    _bar_handles: bar_items::BarHandles,
}

impl WeechatPlugin for Discord {
    // Note: We cannot use on_main (or plugin_print)
    fn init(weechat: Weechat, args: ArgsWeechat) -> WeechatResult<Self> {
        let args: Vec<_> = args.collect();

        // Copy so we can print stuff and pass the discord object around
        let weechat_copy: &Weechat = unsafe { &*{ &weechat as *const _ } };

        let _sync_handle = sync::init(&weechat);
        let _hook_handles = hook::init(&weechat);
        let _bar_handles = bar_items::init(&weechat);
        let config = config::init(&weechat);

        let autostart = config.autostart.value();
        let irc_mode = config.irc_mode.value();
        let token = config.token.value().into_owned();
        let token = if token.starts_with("${sec.data") {
            weechat.eval_string_expression(&token).map(Cow::into_owned)
        } else {
            Some(token)
        };

        let weecord = Discord {
            weechat,
            config,
            _sync_handle,
            _hook_handles,
            _bar_handles,
        };

        if !args.contains(&"-a".to_owned()) && autostart {
            if let Some(t) = token {
                if !t.is_empty() {
                    discord::init(&weecord, &t, irc_mode);
                } else {
                    weechat_copy.print("Error: weecord.main.token is not set. To set it, run:");
                    weechat_copy.print("/discord token 123456789ABCDEF");
                }
            } else {
                weechat_copy
                    .print("Error: failed to evaluate token option, expected valid ${sec.data...}");
            }
        }

        Ok(weecord)
    }
}

impl Drop for Discord {
    fn drop(&mut self) {
        // TODO: Why is the config file not saved on quit?
        self.config.config.write()
    }
}

impl std::ops::Deref for Discord {
    type Target = Weechat;

    fn deref(&self) -> &Self::Target {
        &self.weechat
    }
}

weechat_plugin!(
    Discord,
    name: "weecord",
    author: "Noskcaj19",
    description: "Discord integration for weechat",
    version: "0.2.0",
    license: "MIT"
);

pub fn plugin_print(msg: &str) {
    let msg = msg.to_owned();
    on_main(move |weechat| weechat.print(&format!("discord: {}", msg)))
}
