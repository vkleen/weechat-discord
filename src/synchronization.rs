use serenity::prelude::Mutex;
use std::sync::{Arc, Barrier};

lazy_static! {
    pub static ref M1: Mutex<()> = Mutex::new(());
    pub static ref BARRIER: Arc<Barrier> = Arc::new(Barrier::new(1));
}

/// A macro that will cause all code inside to be executed
/// on weechats' main thread.
///
/// This _must_ be used to wrap all operations that cross the ffi threshold
/// TODO: Make this work at the type level
macro_rules! on_main {
    ($block:block) => {{
        let _lock = $crate::synchronization::M1.lock();

        let barrier = $crate::synchronization::BARRIER.clone();
        barrier.wait();

        let __tmp = $block;

        barrier.wait();

        __tmp
    }};
}
