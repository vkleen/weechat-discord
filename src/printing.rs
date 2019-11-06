use crate::discord::formatting;
use serenity::model::prelude::*;
use weechat::{Buffer, Weechat};

// TODO: Rework args
// TODO: Color things
pub fn print_msg(weechat: &Weechat, buffer: &Buffer, msg: &Message, notify: bool) {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let is_private = if let Some(channel) = msg.channel(ctx) {
        if let Channel::Private(_) = channel {
            true
        } else {
            false
        }
    } else {
        false
    };

    let self_mentioned = msg.mentions_user_id(ctx.cache.read().user.id);

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

    let maybe_guild = buffer
        .get_localvar("guildid")
        .and_then(|id| id.parse::<u64>().ok().map(GuildId));

    let mut opts = serenity::utils::ContentSafeOptions::new()
        .clean_here(false)
        .clean_everyone(false);
    if let Some(guild) = maybe_guild {
        opts = opts.display_as_member_from(guild);
    }
    let mut msg_content = serenity::utils::content_safe(&ctx.cache, &msg.content, &opts);

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

    let display_name = maybe_guild.and_then(|id| {
        ctx.cache
            .read()
            .member(id, msg.author.id)
            .map(|member| member.display_name().to_string())
    });

    let author = display_name.unwrap_or_else(|| msg.author.name.to_owned());

    use MessageType::*;
    match msg.kind {
        Regular => {
            buffer.print_tags_dated(
                msg.timestamp.timestamp(),
                &tags,
                &format!(
                    "{}\t{}",
                    author,
                    formatting::discord_to_weechat(weechat, &msg_content)
                ),
            );
        }

        _ => {
            let (prefix, body) = match msg.kind {
                GroupRecipientAddition | MemberJoin => {
                    ("join", format!("{} joined the group.", author))
                }
                GroupRecipientRemoval => ("quit", format!("{} left the group.", author)),
                GroupNameUpdate => (
                    "network",
                    format!("{} changed the channel name: {}.", author, msg.content),
                ),
                GroupCallCreation => ("network", format!("{} started a call.", author)),
                GroupIconUpdate => ("network", format!("{} changed the channel icon.", author)),
                PinsAdd => (
                    "network",
                    format!("{} pinned a message to this channel", author),
                ),
                NitroBoost => (
                    "network",
                    format!("{} boosted this channel with nitro", author),
                ),
                NitroTier1 => (
                    "network",
                    format!("This channel has achieved nitro level 1"),
                ),
                NitroTier2 => (
                    "network",
                    format!("This channel has achieved nitro level 2"),
                ),
                NitroTier3 => (
                    "network",
                    format!("This channel has achieved nitro level 3"),
                ),
                Regular | __Nonexhaustive => unreachable!(),
            };
            buffer.print_tags_dated(
                msg.timestamp.timestamp(),
                &tags,
                &(weechat.get_prefix(prefix).into_owned() + &body),
            );
        }
    };
}
