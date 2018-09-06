use ffi::Buffer;
use serenity::model::id::{ChannelId, GuildId};

pub fn buffer_id_from_guild(id: &GuildId) -> String {
    format!("G{}", id)
}

pub fn buffer_id_from_channel(id: &ChannelId) -> String {
    format!("C{}", id)
}

pub fn buffer_is_muted(buffer: &Buffer) -> bool {
    if let Some(muted) = buffer.get("localvar_muted") {
        &muted == "1"
    } else {
        false
    }
}
