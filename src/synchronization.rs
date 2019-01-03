use serenity::prelude::Mutex;

lazy_static! {
    pub static ref MAIN_ENTRY_MUTEX: Mutex<()> = Mutex::new(());
    // pub static ref CHAN: (
    //     crossbeam_channel::Sender<()>,
    //     crossbeam_channel::Receiver<()>
    // ) = crossbeam_channel::bounded(0);
    pub static ref WEE_SYNC: WeechatSync = WeechatSync::new();
}

pub struct WeechatSync(
    crossbeam_channel::Sender<()>,
    crossbeam_channel::Receiver<()>,
);

impl WeechatSync {
    pub fn new() -> WeechatSync {
        let (tx, rx) = crossbeam_channel::bounded(0);
        WeechatSync(tx, rx)
    }

    pub fn try_recv(&self) -> Result<(), crossbeam_channel::TryRecvError> {
        self.1.try_recv()
    }

    pub fn recv(&self) {
        let _ = self.1.recv();
    }

    pub fn send(&self) {
        let _ = self.0.send(());
    }

    pub fn lock(&self) -> WeechatSyncGuard {
        WeechatSyncGuard::new(self)
    }
}

pub struct WeechatSyncGuard<'a>(&'a WeechatSync);

impl<'a> WeechatSyncGuard<'a> {
    pub fn new(sync: &'a WeechatSync) -> WeechatSyncGuard {
        sync.send();
        WeechatSyncGuard(sync)
    }
}

impl<'a> Drop for WeechatSyncGuard<'a> {
    fn drop(&mut self) {
        self.0.recv()
    }
}

/// A macro that will cause all code inside to be executed
/// on weechats' main thread.
///
/// This _must_ be used to wrap all operations that cross the ffi threshold
/// TODO: Make this work at the type level
macro_rules! on_main {
    ($block:block) => {{
        let __lock = $crate::synchronization::MAIN_ENTRY_MUTEX.lock();

        let __weechat_sync_guard = $crate::synchronization::WEE_SYNC.lock();

        let val = { $block };

        drop(__weechat_sync_guard);
        drop(__lock);

        val
    }};
}
