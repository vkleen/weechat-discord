use crate::utils::BufferExt;
use serenity::{
    cache::CacheRwLock,
    model::{channel::Message, id::MessageId},
};
use std::{cell::RefCell, ops::Deref, sync::Arc};
use weechat::Buffer;

/// MessageRenderer wraps a weechat buffer and facilitates editing the buffer and drawing the
/// messages
pub struct MessageManager {
    buffer: Buffer,
    messages: Arc<RefCell<Vec<Message>>>,
}

impl MessageManager {
    /// Create a new MessageManager from a buffer
    pub fn new(buffer: Buffer) -> MessageManager {
        MessageManager {
            buffer,
            messages: Arc::new(RefCell::new(Vec::new())),
        }
    }

    /// Format and print message to the buffer
    fn print_msg(&self, cache: &CacheRwLock, msg: &Message, notify: bool) {
        let weechat = self.buffer.get_weechat();
        let maybe_guild = self.buffer.guild_id();
        let (prefix, content) = formatting_utils::render_msg(cache, &weechat, msg, maybe_guild);
        self.buffer.print_tags_dated(
            msg.timestamp.timestamp(),
            &formatting_utils::msg_tags(cache, msg, notify).join(","),
            &format!("{}\t{}", prefix, content),
        );
    }

    /// Clear the buffer and reprint all messages
    fn redraw_buffer(&self, cache: &CacheRwLock) {
        self.buffer.clear();
        for message in self.messages.borrow().iter() {
            self.print_msg(cache, &message, false);
        }
    }

    /// Add a message to the end of a buffer (chronologically)
    pub fn add_message(&self, cache: &CacheRwLock, msg: &Message, notify: bool) {
        self.print_msg(cache, msg, notify);
        self.messages.borrow_mut().push(msg.clone());
    }

    // Overwrite a previously printed message, has no effect if the message does not exist
    pub fn replace_message(&self, cache: &CacheRwLock, id: &MessageId, msg: &Message) {
        if let Some(old_msg) = self
            .messages
            .borrow_mut()
            .iter_mut()
            .find(|it| &it.id == id)
        {
            *old_msg = msg.clone();
        }
        // Using hdata to edit the line might be more efficient Would still use redrawing as a fall
        // back in the event that the edit has a different amount of lines
        self.redraw_buffer(cache);
    }

    /// Delete a previously printed message, has no effect if the message does not exist
    pub fn delete_message(&self, cache: &CacheRwLock, id: &MessageId) {
        let index = self.messages.borrow().iter().position(|it| &it.id == id);
        if let Some(index) = index {
            self.messages.borrow_mut().remove(index);
        };
        // Performance, see comment above
        self.redraw_buffer(cache);
    }
}

impl Deref for MessageManager {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

mod formatting_utils {
    use crate::{discord::formatting, utils::format_nick_color};
    use serenity::{cache::CacheRwLock, model::prelude::*};
    use weechat::Weechat;

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
            let edited_text = weechat.color("8").into_owned()
                + " (edited)"
                + &weechat.color("reset").into_owned();
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
                msg_content.push('▎');
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
                msg_content.push_str(
                    &title
                        .lines()
                        .fold(String::new(), |acc, x| format!("{}▎{}\n", acc, x)),
                );
                msg_content.push('\n');
            }
            if let Some(ref description) = embed.description {
                msg_content.push_str(
                    &description
                        .lines()
                        .fold(String::new(), |acc, x| format!("{}▎{}\n", acc, x)),
                );
                msg_content.push('\n');
            }
            for field in &embed.fields {
                msg_content.push_str(&field.name);
                msg_content.push_str(
                    &field
                        .value
                        .lines()
                        .fold(String::new(), |acc, x| format!("{}▎{}\n", acc, x)),
                );
                msg_content.push('\n');
            }
            if let Some(ref footer) = embed.footer {
                msg_content.push_str(
                    &footer
                        .text
                        .lines()
                        .fold(String::new(), |acc, x| format!("{}▎{}\n", acc, x)),
                );
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

    pub fn author_display_name(
        cache: &CacheRwLock,
        msg: &Message,
        guild: Option<GuildId>,
    ) -> String {
        let display_name = guild.and_then(|id| {
            cache
                .read()
                .member(id, msg.author.id)
                .map(|member| member.display_name().to_string())
        });
        display_name.unwrap_or_else(|| msg.author.name.to_owned())
    }
}
