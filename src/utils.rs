use serenity::model::id::{ChannelId, GuildId};

pub fn buffer_id_from_guild(id: &GuildId) -> String {
    format!("G{}", id)
}

pub fn buffer_id_from_channel(id: &ChannelId) -> String {
    format!("C{}", id)
}
