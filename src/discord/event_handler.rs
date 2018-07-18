use ffi::Buffer;

use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::CACHE;

use super::formatting;

pub struct Handler();

impl EventHandler for Handler {
    // Called when a message is received
    fn message(&self, _: Context, msg: Message) {
        let string_channel = msg.channel_id.0.to_string();

        if let Some(buffer) = Buffer::search(&string_channel) {
            let is_private = if let Some(channel) = msg.channel() {
                if let Channel::Private(_) = channel {
                    true
                } else {
                    false
                }
            } else {
                false
            };

            let self_mentioned = msg.mentions_user_id(CACHE.read().user.id);

            let tags = {
                let mut tags = Vec::new();
                if self_mentioned {
                    tags.push("notify_highlight");
                } else if is_private {
                    tags.push("notify_private");
                } else {
                    tags.push("notify_message");
                };
                tags.join(",")
            };

            buffer.print_tags(
                &tags,
                &format!(
                    "{}\t{}",
                    msg.author.name,
                    formatting::discord_to_weechat(&msg.content_safe())
                ),
            );
        }
    }

    // fn message_delete(&self, _: Context, channel: ChannelId, message: MessageId) {}

    // fn message_delete_bulk(&self, _: Context, channel: ChannelId, messages: Vec<MessageId>) {}

    // fn message_update(&self, _: Context, update: event::MessageUpdateEvent) {}

    // fn channel_update(&self, _: Context, _: Option<Channel>, _: Channel) {}

    // fn typing_start(&self, _: Context, event: TypingStartEvent) {}
}
