use serenity::prelude::Mutex;

lazy_static! {
    pub static ref MAIN_ENTRY_MUTEX: Mutex<()> = Mutex::new(());
    pub static ref CHAN: (
        crossbeam_channel::Sender<()>,
        crossbeam_channel::Receiver<()>
    ) = crossbeam_channel::bounded(0);
}

/// A macro that will cause all code inside to be executed
/// on weechats' main thread.
///
/// This _must_ be used to wrap all operations that cross the ffi threshold
/// TODO: Make this work at the type level
macro_rules! on_main {
    ($block:block) => {{
        let __on_main_fn = || $block;
        let _lock = $crate::synchronization::MAIN_ENTRY_MUTEX.lock();

        let _ = $crate::synchronization::CHAN.0.send(());
        let val = __on_main_fn();
        let _ = $crate::synchronization::CHAN.1.recv();

        val
    }};
}
