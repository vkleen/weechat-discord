#[macro_use]
extern crate weechat;

#[macro_use]
mod synchronization;
mod buffers;
mod discord;
mod ffi;
mod hook;
mod printing;
mod utils;

use crate::ffi::get_option;
pub use crate::ffi::MAIN_BUFFER;

use weechat::{ArgsWeechat, Weechat, WeechatPlugin, WeechatResult};

struct Weecord {
    weechat: Weechat,
    _handles: hook::HookHandles,
}

impl Weecord {
    pub fn plugin_print(&self, message: &str) {
        self.weechat.print(&format!("weecord: {}", message))
    }
}

impl WeechatPlugin for Weecord {
    fn init(mut weechat: Weechat, args: ArgsWeechat) -> WeechatResult<Self> {
        let args: Vec<_> = args.collect();

        // Hack to bridge the two ffi modules
        ffi::set_plugin(weechat.as_ptr() as *mut std::ffi::c_void);

        let _handles = hook::init(&weechat).expect("Failed to create signal hooks");

        if let Some(autostart) = get_option("autostart") {
            if !args.contains(&"-a".to_owned()) && autostart == "true" {
                if let Some(t) = ffi::get_option("token") {
                    let t = if t.starts_with("${sec.data") {
                        weechat.eval_string_expression(&t)
                    } else {
                        &t
                    };
                    discord::init(&t, utils::get_irc_mode());
                } else {
                    plugin_print("Error: plugins.var.weecord.token unset. Run:");
                    plugin_print("/discord token 123456789ABCDEF");
                }
            }
        }

        Ok(Weecord { weechat, _handles })
    }
}

weechat_plugin!(
    Weecord,
    name: b"weecord\0"; 8,
    author:  b"khyperia <khyperia@live.com>\0"; 29,
    description: b"Discord support for weechat\0"; 28,
    version: b"0.1\0"; 4,
    license: b"MIT\0"; 4
);

pub fn plugin_print(message: &str) {
    MAIN_BUFFER.print(&format!("weecord: {}", message));
}
