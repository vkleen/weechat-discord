use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

lazy_static! {
    pub static ref TX: Mutex<Option<Sender<()>>> = Mutex::new(None);
    pub static ref RX: Mutex<Option<Receiver<()>>> = Mutex::new(None);
}

pub fn init() {
    let (tx, rx) = channel();
    *TX.lock().unwrap() = Some(tx);
    *RX.lock().unwrap() = Some(rx);
}

/// A macro that will cause all code inside to be executed
/// on weechats' main thread.
///
/// This _must_ be used to wrap all operations that cross the ffi threshold
/// TODO: Make this work at the type level
macro_rules! on_main {
    ($block:block) => {{
        if let Ok(rx) = $crate::synchronization::RX.lock() {
            $crate::MAIN_BUFFER.send_trigger_hook("main_thread_lock");
            if let Some(ref rx) = *rx {
                rx.recv().unwrap();
            }
        }
        let __tmp = $block;

        if let Ok(tx) = $crate::synchronization::TX.lock() {
            if let Some(ref tx) = *tx {
                tx.send(()).unwrap();
            }
        }
        __tmp
    }};
}
