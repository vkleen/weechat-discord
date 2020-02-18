use crate::{
    discord::formatting,
    utils::{BufferExt,format_nick_color},
};
use serenity::{cache::CacheRwLock, model::prelude::*};
use weechat::{hdata::HDataPointer, Buffer, HasHData, Weechat};

pub fn render_msg(
    cache: &CacheRwLock,
    weechat: &Weechat,
    msg: &Message,
    guild: Option<GuildId>,
) -> (String, String) {
    let mut opts = serenity::utils::ContentSafeOptions::new()
        .clean_here(false)
        .clean_everyone(false);
    if let Some(guild) = guild {
        opts = opts.display_as_member_from(guild);
    }
    let mut msg_content = serenity::utils::content_safe(&cache, &msg.content, &opts);
    msg_content = crate::utils::clean_emojis(&msg_content);
    if msg.edited_timestamp.is_some() {
        let edited_text =
            weechat.color("8").into_owned() + " (edited)" + &weechat.color("reset").into_owned();
        msg_content.push_str(&edited_text);
    }

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
            msg_content.push_str(&format!(
                "{}{}{}",
                weechat.color("bold"),
                author.name,
                weechat.color("reset"),
            ));
            if let Some(url) = &author.url {
                msg_content.push_str(&format!(" ({})", url));
            }
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

    let author = format_nick_color(weechat, &author_display_name(cache, &msg, guild));

    use MessageType::*;
    if let Regular = msg.kind {
        (
            author,
            formatting::discord_to_weechat(weechat, &msg_content),
        )
    } else {
        let (prefix, body) = match msg.kind {
            GroupRecipientAddition | MemberJoin => {
                ("join", format!("{} joined the group.", author))
            },
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
                "This channel has achieved nitro level 1".to_string(),
            ),
            NitroTier2 => (
                "network",
                "This channel has achieved nitro level 2".to_string(),
            ),
            NitroTier3 => (
                "network",
                "This channel has achieved nitro level 3".to_string(),
            ),
            Regular | __Nonexhaustive => unreachable!(),
        };
        (weechat.get_prefix(prefix).into_owned(), body)
    }
}

pub fn author_display_name(cache: &CacheRwLock, msg: &Message, guild: Option<GuildId>) -> String {
    let display_name = guild.and_then(|id| {
        cache
            .read()
            .member(id, msg.author.id)
            .map(|member| member.display_name().to_string())
    });
    display_name.unwrap_or_else(|| msg.author.name.to_owned())
}

pub fn msg_tags(cache: &CacheRwLock, msg: &Message, notify: bool) -> Vec<String> {
    let is_private = if let Some(channel) = msg.channel(cache) {
        if let Channel::Private(_) = channel {
            true
        } else {
            false
        }
    } else {
        false
    };

    let self_mentioned = msg.mentions_user_id(cache.read().user.id);

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

    tags.into_iter().map(ToString::to_string).collect()
}

// TODO: Color things
pub fn print_msg(weechat: &Weechat, buffer: &Buffer, msg: &Message, notify: bool) {
    let ctx = match crate::discord::get_ctx() {
        Some(ctx) => ctx,
        _ => return,
    };
    let maybe_guild = buffer.guild_id();

    let (prefix, content) = render_msg(&ctx.cache, weechat, msg, maybe_guild);
    let timestamp = msg.timestamp.timestamp();
    let tags = msg_tags(&ctx.cache, msg, notify).join(",");
    buffer.print_tags_dated(timestamp, &tags, &format!("{}\t{}", prefix, content));
}

// Use the `date_printed` hdata field to store the message id in the last message
pub fn inject_msg_id(msg_id: MessageId, buffer: &Buffer) {
    let buffer_hdata = buffer.get_hdata("buffer").unwrap();
    let lines_ptr: HDataPointer = buffer_hdata.get_var("own_lines").unwrap();
    let lines_hdata = lines_ptr.get_hdata("lines").unwrap();
    let mut maybe_last_line_ptr = lines_hdata.get_var::<HDataPointer>("last_line");

    while let Some(last_line_ptr) = maybe_last_line_ptr {
        let last_line_hdata = last_line_ptr.get_hdata("line").unwrap();
        let line_data_ptr: HDataPointer = last_line_hdata.get_var("data").unwrap();

        let line_data_hdata = line_data_ptr.get_hdata("line_data").unwrap();
        line_data_hdata.update_var("date_printed", msg_id.0.to_string());

        if let Some(prefix) = unsafe { line_data_hdata.get_string_unchecked("prefix") } {
            if !prefix.is_empty() {
                break;
            }
        }

        maybe_last_line_ptr = last_line_ptr.advance(&last_line_hdata, -1);
    }
}
