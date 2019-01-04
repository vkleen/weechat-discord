use crate::{buffers, ffi::Buffer, printing, utils};
use serenity::{model::prelude::*, prelude::*, CACHE};
use std::sync::{mpsc::Sender, Arc};

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
        on_main! {{
            if let Some(buffer) = Buffer::search(&string_channel) {
                let muted = utils::buffer_is_muted(&buffer);
                let notify = !msg.is_own() && !muted;
                printing::print_msg(&buffer, &msg, notify);
            } else {
                match msg.channel_id.to_channel() {
                    chan @ Ok(Channel::Private(_)) => {
                        if let Some(buffer) = Buffer::search(&string_channel) {
                            let muted = utils::buffer_is_muted(&buffer);
                            let notify = !msg.is_own() && !muted;
                            printing::print_msg(&buffer, &msg, notify);
                        } else {
                            // TODO: Implement "switch_to"
                            buffers::create_buffer_from_dm(
                                chan.unwrap(),
                                &CACHE.read().user.name,
                                false,
                            );
                        }
                    }
                    chan @ Ok(Channel::Group(_)) => {
                        if let Some(buffer) = Buffer::search(&string_channel) {
                            let muted = utils::buffer_is_muted(&buffer);
                            let notify = !msg.is_own() && !muted;
                            printing::print_msg(&buffer, &msg, notify);
                        } else {
                            buffers::create_buffer_from_group(chan.unwrap(), &CACHE.read().user.name);
                        }
                    }
                    _ => {}
                }
            }
        }};
    }

    // fn message_delete(&self, _: Context, channel: ChannelId, message: MessageId) {}

    // fn message_delete_bulk(&self, _: Context, channel: ChannelId, messages: Vec<MessageId>) {}

    // fn message_update(&self, _: Context, update: event::MessageUpdateEvent) {}

    // fn channel_update(&self, _: Context, _: Option<Channel>, _: Channel) {}

    // TODO Why are we not getting these events
    // fn typing_start(&self, _: Context, event: TypingStartEvent) {}
}
