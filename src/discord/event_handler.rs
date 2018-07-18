use ffi::Buffer;

use serenity::model::prelude::*;
use serenity::prelude::*;

use super::formatting;

pub struct Handler;

impl EventHandler for Handler {
    // Called when a message is received
    fn message(&self, _: Context, msg: Message) {
        let string_channel = msg.channel_id.0.to_string();

        if let Some(buffer) = Buffer::search(&string_channel) {
            if msg.is_own() {
                formatting::display_msg(&buffer, &msg, false);
            } else {
                formatting::display_msg(&buffer, &msg, true);
            }
        }
    }

    // fn message_delete(&self, _: Context, channel: ChannelId, message: MessageId) {}

    // fn message_delete_bulk(&self, _: Context, channel: ChannelId, messages: Vec<MessageId>) {}

    // fn message_update(&self, _: Context, update: event::MessageUpdateEvent) {}

    // fn channel_update(&self, _: Context, _: Option<Channel>, _: Channel) {}

    // fn typing_start(&self, _: Context, event: TypingStartEvent) {}
}