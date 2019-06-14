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
}

impl Weecord {
    pub fn init(&mut self, args: ArgsWeechat) {
        let args: Vec<_> = args.collect();

        // Hack to bridge the two ffi modules
        ffi::set_plugin(self.weechat.as_ptr() as *mut std::ffi::c_void);

        hook::init();

        if let Some(autostart) = get_option("autostart") {
            if !args.contains(&"-a".to_owned()) {
                if autostart == "true" {
                    if let Some(t) = ffi::get_option("token") {
                        let t = if t.starts_with("${sec.data") {
                            self.weechat.eval_string_expression(&t)
                        } else {
                            &t
                        };
                        discord::init(&t, utils::get_irc_mode());
                    } else {
                        self.plugin_print("Error: plugins.var.weecord.token unset. Run:");
                        self.plugin_print("/discord token 123456789ABCDEF");
                    }
                }
            }
        }
    }

    pub fn plugin_print(&self, message: &str) {
        self.weechat.print(&format!("weecord: {}", message))
    }
}

impl WeechatPlugin for Weecord {
    fn init(weechat: Weechat, args: ArgsWeechat) -> WeechatResult<Self> {
        let mut weecord = Weecord { weechat };
        weecord.init(args);
        Ok(weecord)
    }
}

impl Drop for Weecord {
    fn drop(&mut self) {
        hook::destroy();
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
