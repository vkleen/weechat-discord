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

pub fn rgb_to_ansi(color: serenity::utils::Colour) -> u8 {
    let r = (u16::from(color.r()) * 5 / 255) as u8;
    let g = (u16::from(color.g()) * 5 / 255) as u8;
    let b = (u16::from(color.b()) * 5 / 255) as u8;
    16 + 36 * r + 6 * g + b
}
