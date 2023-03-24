use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// Debounces events as long as it is busy.
///
/// Instead of using a fixed time period to debounce events, it debounces events
/// as long as it is busy working on an event.
///
/// The idea is that a producer can push events to the debouncer, which take some time processing.
/// While processing, only the most recent event will be recorded, and executed after the
/// previous event finished.
///
/// It is intended for scenarios where it is not important to execute a task for all events, but
/// (ideally) as soon as possible, at least once after an event was published, but not process
/// for events that are obsolete due to succeeding events.
pub struct BusyDebouncer<T> {
    notify: Arc<Notify>,
    data: Arc<Mutex<Option<T>>>,
}

impl<T> BusyDebouncer<T> {
    pub fn new<C, F>(context: C, handler: F) -> Self
    where
        C: Send + 'static,
        T: Send + Sync + 'static,
        for<'a> F: Fn(&'a mut C, T) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'a>>
            + Send
            + Sync
            + 'static,
    {
        let notify = Arc::new(Notify::new());
        let data = Arc::new(Mutex::new(None));

        {
            let notify = notify.clone();
            let data = data.clone();
            tokio::spawn(async move {
                let mut context = context;
                loop {
                    notify.notified().await;
                    let next = data.lock().unwrap().take();
                    match next {
                        Some(event) => {
                            handler(&mut context, event).await;
                        }
                        None => break,
                    }
                }
            });
        }

        Self { notify, data }
    }

    /// Push a new task to the debouncer.
    ///
    /// This call will return immediately, and might spawn the event now, at a later time, or never.
    pub fn push(&self, event: T) {
        self.send(Some(event));
    }

    fn send(&self, msg: Option<T>) {
        *self.data.lock().unwrap() = msg;
        self.notify.notify_one();
    }
}

impl<T> Drop for BusyDebouncer<T> {
    fn drop(&mut self) {
        self.send(None);
    }
}
