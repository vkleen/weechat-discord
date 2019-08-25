use crate::discord::formatting;
use lazy_static::lazy_static;
use regex::Regex;
use serenity::cache::CacheRwLock;
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

    let mut msg_content = humanize_msg(&ctx.cache, msg);

    for attachment in &msg.attachments {
        if !msg_content.is_empty() {
            msg_content.push('\n');
        }
        if attachment.width.is_some() {
            // it's an image
            if let Ok(ref data) = attachment.download() {
                if let Some(ref str) = draw_img(weechat, data) {
                    msg_content.push_str(str);
                    continue;
                }
            }
        }
        msg_content.push_str(&attachment.proxy_url);
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

    let maybe_guild = buffer.get_localvar("guildid");
    let display_name = maybe_guild.and_then(|id| {
        id.parse::<u64>().ok().map(GuildId).and_then(|id| {
            ctx.cache
                .read()
                .member(id, msg.author.id)
                .map(|member| member.display_name().to_string())
        })
    });

    let author = display_name.unwrap_or_else(|| msg.author.name.to_owned());

    buffer.print_tags_dated(
        msg.timestamp.timestamp(),
        &tags,
        &format!(
            "{}\t{}",
            author,
            &msg_content //            formatting::discord_to_weechat(weechat, &msg_content)
        ),
    );
}

/// Convert discords mention formatting to real names
///
/// Eg, convert user mentions in the form of `<@343888830585372672>` to `@Noskcaj#0804`
/// as well as roles and channels
fn humanize_msg(cache: impl AsRef<CacheRwLock>, msg: &Message) -> String {
    let mut msg_content = msg.content_safe(cache.as_ref());

    // TODO: Report content_safe() bug
    // TODO: Use nicknames instead of user names
    for u in &msg.mentions {
        let mut at_distinct = String::with_capacity(38);
        at_distinct.push('@');
        at_distinct.push_str(&u.name);
        at_distinct.push('#');
        let mention = u.mention().replace("<@", "<@!");
        use std::fmt::Write;
        let _ = write!(at_distinct, "{:04}", u.discriminator);
        msg_content = msg_content.replace(&mention, &at_distinct);
    }

    lazy_static! {
        static ref CHANNEL_REGEX: Regex = Regex::new(r"<#(\d+)>").unwrap();
        static ref USERNAME_REGEX: Regex = Regex::new(r"<@(\d+)>").unwrap();
    }

    let mut edits = Vec::new();
    // TODO: Why do member joins not replace the user id correctly?
    if msg.kind != MessageType::MemberJoin {
        for cap in USERNAME_REGEX.captures_iter(&msg_content) {
            edits.push((
                cap.get(0).unwrap().as_str().to_owned(),
                format!("@deleted-user"),
            ));
        }
    }

    for cap in CHANNEL_REGEX.captures_iter(&msg_content) {
        let i = cap.get(1).unwrap();
        let channel_id = i.as_str().parse::<u64>().unwrap();
        let replacement =
            if let Some(channel) = ChannelId(channel_id).to_channel_cached(cache.as_ref()) {
                crate::utils::channel_name(&channel)
            } else {
                "unknown-channel".into()
            };
        edits.push((
            cap.get(0).unwrap().as_str().to_owned(),
            format!("#{}", replacement),
        ));
    }

    for edit in &edits {
        msg_content = msg_content.replace(&edit.0, &edit.1);
    }

    msg_content
}

fn draw_img(weechat: &Weechat, data: &[u8]) -> Option<String> {
    let img = image::load_from_memory(data).ok()?;

    let img = resize_image(&img, (4, 8), (900, 25));

    let render = termimage::render(img, true, 2);

    let mut out = String::new();

    for x in render {
        for y in x {
            let fg = termimage::rgb_to_ansi(y.fg).0;
            let bg = termimage::rgb_to_ansi(y.bg).0;
            out.push_str(&format!(
                "{}{}{}",
                weechat.color(&format!("{},{}", fg, bg)),
                y.ch,
                weechat.color("reset")
            ))
        }
        out.push('\n');
    }

    Some(out)
}

/// Resizes an image to fit within a max size, then scales an image to fit within a block size
fn resize_image(
    img: &image::DynamicImage,
    cell_size: (u32, u32),
    max_size: (u16, u16),
) -> image::DynamicImage {
    use image::GenericImageView;
    let img = img.resize(
        (u32::from(max_size.0)) * cell_size.0,
        (u32::from(max_size.1)) * cell_size.1,
        image::FilterType::Nearest,
    );

    img.resize_exact(
        closest_mult(img.width(), cell_size.0),
        closest_mult(img.height(), cell_size.1),
        image::FilterType::Nearest,
    )
}

/// Returns the closest multiple of a base
fn closest_mult(x: u32, base: u32) -> u32 {
    base * ((x as f32) / base as f32).round() as u32
}
