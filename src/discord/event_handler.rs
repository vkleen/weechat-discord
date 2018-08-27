use ffi::Buffer;
use {buffers, printing, utils};

use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::CACHE;

use std::sync::mpsc::Sender;
use std::sync::Arc;

pub enum WeecordEvent {
    Ready(::serenity::model::gateway::Ready),
}

pub struct Handler(pub Arc<Mutex<Sender<WeecordEvent>>>);

impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        let _ = self.0.lock().send(WeecordEvent::Ready(ready));
    }

    // Called when a message is received
    fn message(&self, _: Context, msg: Message) {
        let string_channel = utils::buffer_id_from_channel(&msg.channel_id);
        if let Some(buffer) = Buffer::search(&string_channel) {
            if msg.is_own() {
                printing::print_msg(&buffer, &msg, false);
            } else {
                printing::print_msg(&buffer, &msg, true);
            }
        } else {
            match msg.channel_id.to_channel() {
                chan @ Ok(Channel::Private(_)) => {
                    if let Some(buffer) = Buffer::search(&string_channel) {
                        if msg.is_own() {
                            printing::print_msg(&buffer, &msg, false);
                        } else {
                            printing::print_msg(&buffer, &msg, true);
                        }
                    } else {
                        buffers::create_buffer_from_dm(
                            chan.unwrap(),
                            &CACHE.read().user.name,
                            false,
                        );
                    }
                }
                chan @ Ok(Channel::Group(_)) => {
                    if let Some(buffer) = Buffer::search(&string_channel) {
                        if msg.is_own() {
                            printing::print_msg(&buffer, &msg, false);
                        } else {
                            printing::print_msg(&buffer, &msg, true);
                        }
                    } else {
                        buffers::create_buffer_from_group(chan.unwrap(), &CACHE.read().user.name);
                    }
                }
                _ => {}
            }
        }
    }

    // fn message_delete(&self, _: Context, channel: ChannelId, message: MessageId) {}

    // fn message_delete_bulk(&self, _: Context, channel: ChannelId, messages: Vec<MessageId>) {}

    // fn message_update(&self, _: Context, update: event::MessageUpdateEvent) {}

    // fn channel_update(&self, _: Context, _: Option<Channel>, _: Channel) {}

    // TODO: Why are we not getting these events
    // fn typing_start(&self, _: Context, event: TypingStartEvent) {}
}
