use crate::ffi::{get_option, Buffer};
use serenity::model::id::{ChannelId, GuildId};

pub fn buffer_id_for_guild(id: GuildId) -> String {
    format!("G{}", id.0)
}

pub fn buffer_id_for_channel(guild_id: Option<GuildId>, channel_id: ChannelId) -> String {
    let guild = guild_id
        .map(|g| g.0.to_string())
        .unwrap_or_else(|| "None".to_string());
    format!("G{}.#C{}", guild, channel_id.0)
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

pub fn get_irc_mode() -> bool {
    get_option("irc_mode").map(|x| x == "true").unwrap_or(false)
}
