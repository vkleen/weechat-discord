use crate::{buffers, ffi::Buffer, printing, utils};
use serenity::client::bridge::gateway::Message as WsMessage;
use serenity::{model::prelude::*, prelude::*};
use std::sync::{mpsc::Sender, Arc};

pub enum WeecordEvent {
    Ready(::serenity::model::gateway::Ready),
}

pub struct Handler(pub Arc<Mutex<Sender<WeecordEvent>>>);

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        // Opcode 12 is undocumented "guild sync"
        let data = serde_json::json!({
            "op": 12,
            "d": ready.guilds.iter().map(|g| g.id().0.to_string()).collect::<Vec<_>>()
        });
        ctx.shard
            .websocket_message(WsMessage::text(data.to_string()));
        // Cache seems not to have private channels properly populated
        {
            let mut ctx_lock = ctx.cache.write();
            for (&id, channel) in &ready.private_channels {
                if let Some(pc) = channel.clone().private() {
                    ctx_lock.private_channels.insert(id, pc);
                }
            }
        }
        let _ = self.0.lock().send(WeecordEvent::Ready(ready));
        unsafe {
            crate::discord::CONTEXT = Some(ctx);
        }
    }

    // Called when a message is received
    fn message(&self, ctx: Context, msg: Message) {
        let string_channel = utils::buffer_id_for_channel(msg.guild_id, msg.channel_id);
        on_main! {{
            if let Some(buffer) = Buffer::search(&string_channel) {
                let muted = utils::buffer_is_muted(&buffer);
                let notify = !msg.is_own(ctx.cache) && !muted;
                printing::print_msg(&buffer, &msg, notify);
            } else {
                match msg.channel_id.to_channel(&ctx) {
                    chan @ Ok(Channel::Private(_)) => {
                        if let Some(buffer) = Buffer::search(&string_channel) {
                            let muted = utils::buffer_is_muted(&buffer);
                            let notify = !msg.is_own(ctx.cache) && !muted;
                            printing::print_msg(&buffer, &msg, notify);
                        } else {
                            // TODO: Implement "switch_to"
                            buffers::create_buffer_from_dm(
                                chan.unwrap(),
                                &ctx.cache.read().user.name,
                                false,
                            );
                        }
                    }
                    chan @ Ok(Channel::Group(_)) => {
                        if let Some(buffer) = Buffer::search(&string_channel) {
                            let muted = utils::buffer_is_muted(&buffer);
                            let notify = !msg.is_own(ctx.cache) && !muted;
                            printing::print_msg(&buffer, &msg, notify);
                        } else {
                            buffers::create_buffer_from_group(chan.unwrap(), &ctx.cache.read().user.name);
                        }
                    }
                    _ => {}
                }
            }
        }};
    }

    fn typing_start(&self, ctx: Context, event: TypingStartEvent) {
        if event.user_id == ctx.cache.read().user.id {
            return;
        }
        let buffer_id = crate::utils::buffer_id_for_channel(event.guild_id, event.channel_id);
        if let Some(buffer) = crate::ffi::Buffer::search(&buffer_id) {
            let prefix = crate::ffi::get_prefix("network").unwrap_or_else(|| "".to_string());
            let user = event
                .user_id
                .to_user_cached(ctx.cache)
                .map(|user| user.read().name.clone())
                .unwrap_or_else(|| "Someone".to_string());
            buffer.print(&format!("{}\t{} is typing", prefix, user));
        }
    }
}
