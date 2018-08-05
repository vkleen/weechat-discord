use ffi::Buffer;
use serenity::model::prelude::*;
use serenity::CACHE;

use discord::format;

// TODO: Rework args
// TODO: Color things
pub fn print_msg(buffer: &Buffer, msg: &Message, notify: bool) {
    let cache_lock = CACHE.read();
    let is_private = if let Some(channel) = msg.channel() {
        if let Channel::Private(_) = channel {
            true
        } else {
            false
        }
    } else {
        false
    };

    let self_mentioned = msg.mentions_user_id(cache_lock.user.id);

    let tags = {
        let mut tags = Vec::new();
        if notify {
            if self_mentioned {
                tags.push("notify_highlight");
            } else if is_private {
                tags.push("notify_private");
            } else {
                tags.push("notify_message");
            };
        } else {
            tags.push("notify_none");
        }

        tags.join(",")
    };

    let mut msg_content = msg.content_safe();

    for attachement in &msg.attachments {
        if !msg_content.is_empty() {
            msg_content.push('\n');
        }
        msg_content.push_str(&attachement.proxy_url);
    }

    for embed in &msg.embeds {
        if !msg_content.is_empty() {
            msg_content.push('\n');
        }
        if let Some(ref author) = embed.author {
            msg_content.push_str(&author.name);
            msg_content.push('\n');
        }
        if let Some(ref title) = embed.title {
            msg_content.push_str(title);
            msg_content.push('\n');
        }
        if let Some(ref description) = embed.description {
            msg_content.push_str(description);
            msg_content.push('\n');
        }
        for field in &embed.fields {
            msg_content.push_str(&field.name);
            msg_content.push_str(&field.value);
            msg_content.push('\n');
        }
        if let Some(ref footer) = embed.footer {
            msg_content.push_str(&footer.text);
            msg_content.push('\n');
        }
    }

    let display_name = buffer.get("localvar_guildid").and_then(|id| {
        id.parse::<u64>().ok().map(|id| GuildId(id)).and_then(|id| {
            cache_lock
                .member(id, msg.author.id)
                .map(|member| member.display_name().to_string())
        })
    });

    let author = display_name.unwrap_or_else(|| msg.author.name.to_owned());

    buffer.print_tags_dated(
        msg.timestamp.timestamp() as i32,
        &tags,
        &format!("{}\t{}", author, format::discord_to_weechat(&msg_content)),
    );
}
