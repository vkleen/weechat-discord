#[macro_use]
extern crate json;

mod bar_items;
mod buffers;
mod command;
mod discord;
mod hook;
mod printing;
mod sync;
mod utils;

pub use sync::{on_main, on_main_blocking};

use weechat::{weechat_plugin, ArgsWeechat, Weechat, WeechatPlugin, WeechatResult};

pub struct Discord {
    weechat: Weechat,
    _sync_handle: sync::SyncHandle,
    _hook_handles: hook::HookHandles,
    _bar_handles: bar_items::BarHandles,
}

impl WeechatPlugin for Discord {
    fn init(weechat: Weechat, args: ArgsWeechat) -> WeechatResult<Self> {
        let args: Vec<_> = args.collect();

        let _sync_handle = sync::init(&weechat);
        let _hook_handles = hook::init(&weechat);
        let _bar_handles = bar_items::init(&weechat);

        if let Some(autostart) = weechat.get_plugin_option("autostart").map(|a| a == "true") {
            if !args.contains(&"-a".to_owned()) && autostart {
                if let Some(t) = weechat.get_plugin_option("token") {
                    let t = if t.starts_with("${sec.data") {
                        weechat.eval_string_expression(&t)
                    } else {
                        &t
                    };
                    discord::init(&weechat, &t, utils::get_irc_mode(&weechat));
                } else {
                    weechat.print("Error: plugins.var.discord.token is not set. To set it, run:");
                    weechat.print("/discord token 123456789ABCDEF");
                }
            }
        }

        Ok(Discord {
            weechat,
            _sync_handle,
            _hook_handles,
            _bar_handles,
        })
    }
}

weechat_plugin!(
    Discord,
    name: b"weecord",
    author:  b"Noskcaj19",
    description: b"Discord integration for weechat",
    version: b"0.2.0",
    license: b"MIT"
);

pub fn plugin_print(msg: &str) {
    let msg = msg.to_owned();
    on_main(move |weechat| weechat.print(&format!("discord: {}", msg)))
}
