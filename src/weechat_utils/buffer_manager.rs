use crate::weechat_utils::message_manager::MessageManager;
use std::{cell::RefCell, collections::HashMap, sync::Arc};
use weechat::Weechat;

/// Manages all buffers for the plugin
pub struct BufferManager {
    weechat: Weechat,
    buffers: RefCell<HashMap<String, Arc<MessageManager>>>,
}

impl BufferManager {
    pub(crate) fn new(weechat: Weechat) -> BufferManager {
        BufferManager {
            weechat,
            buffers: RefCell::new(HashMap::new()),
        }
    }

    pub fn get_buffer(&self, name: &str) -> Option<Arc<MessageManager>> {
        if let Some(buffer) = self.buffers.borrow().get(name) {
            return Some(Arc::clone(buffer));
        }

        if let Some(buffer) = self.weechat.buffer_search("weecord", name) {
            let msg_manager = MessageManager::new(buffer);
            self.buffers
                .borrow_mut()
                .insert(name.into(), Arc::new(msg_manager));
            Some(Arc::clone(self.buffers.borrow().get(name).unwrap()))
        } else {
            None
        }
    }

    pub fn get_or_create_buffer(&self, name: &str) -> Arc<MessageManager> {
        if let Some(buffer) = self.buffers.borrow().get(name) {
            return Arc::clone(buffer);
        }

        if let Some(buffer) = self.weechat.buffer_search("weecord", name) {
            let msg_manager = MessageManager::new(buffer);
            self.buffers
                .borrow_mut()
                .insert(name.into(), Arc::new(msg_manager));
            Arc::clone(self.buffers.borrow().get(name).unwrap())
        } else {
            let msg_manager = MessageManager::new(self.weechat.buffer_new::<(), ()>(
                name,
                Some(|_, b, i| crate::hook::buffer_input(b, &i)),
                None,
                None,
                None,
            ));
            self.buffers
                .borrow_mut()
                .insert(name.into(), Arc::new(msg_manager));
            Arc::clone(self.buffers.borrow().get(name).unwrap())
        }
    }
}
