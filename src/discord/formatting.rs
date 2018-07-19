use ffi::color_codes;
use parsing::{self, Style};

use ffi::Buffer;
use serenity::model::prelude::*;
use serenity::CACHE;

// TODO: Rework args
pub fn display_msg(buffer: &Buffer, msg: &Message, notify: bool) {
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
        &format!("{}\t{}", author, discord_to_weechat(&msg_content)),
    );
}

pub fn discord_to_weechat(msg: &str) -> String {
    let ast = parsing::parse_msg(msg).unwrap_or_else(|| Vec::new());
    let mut result = String::new();
    for node in ast {
        match node {
            Style::Text(txt) => result.push_str(&txt),
            Style::Code(code) => {
                result.push_str(&color_codes("/grey"));
                result.push_str(&code);
                result.push_str(&color_codes("reset"));
            }
            Style::Bold(bold) => {
                result.push_str(&color_codes("bold"));
                result.push_str(&bold);
                result.push_str(&color_codes("-bold"));
            }
            Style::Italic(italic) => {
                result.push_str(&color_codes("italic"));
                result.push_str(&italic);
                result.push_str(&color_codes("-italic"));
            }
            Style::BoldItalics(bold_italic) => {
                result.push_str(&color_codes("bold"));
                result.push_str(&color_codes("italic"));
                result.push_str(&bold_italic);
                result.push_str(&color_codes("-bold"));
                result.push_str(&color_codes("-italic"));
            }
            Style::Underline(under) => {
                result.push_str(&color_codes("underline"));
                result.push_str(&under);
                result.push_str(&color_codes("-underline"));
            }
            Style::UnderlineBold(under_bold) => {
                result.push_str(&color_codes("bold"));
                result.push_str(&color_codes("underline"));
                result.push_str(&under_bold);
                result.push_str(&color_codes("-bold"));
                result.push_str(&color_codes("-underline"));
            }
            Style::UnderlineItalics(under_italics) => {
                result.push_str(&color_codes("italic"));
                result.push_str(&color_codes("underline"));
                result.push_str(&under_italics);
                result.push_str(&color_codes("-italic"));
                result.push_str(&color_codes("-underline"));
            }
            Style::UnderlineBoldItalics(under_bold_italics) => {
                result.push_str(&color_codes("italic"));
                result.push_str(&color_codes("bold"));
                result.push_str(&color_codes("underline"));
                result.push_str(&under_bold_italics);
                result.push_str(&color_codes("-italic"));
                result.push_str(&color_codes("-bold"));
                result.push_str(&color_codes("-underline"));
            }
            Style::Strikethrough(strikethrough) => {
                result.push_str(&color_codes("red"));
                result.push_str("~~");
                result.push_str(&strikethrough);
                result.push_str("~~");
                result.push_str(&color_codes("resetcolor"));
            }
        }
    }
    result
}
