#[cfg(test)]
#[macro_use]
extern crate static_assertions;

use Message::*;
use parking_lot::{Condvar, Mutex};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

enum Message<T> {
    Message(T),
    Terminate,
}

pub struct Sender<T> {
    wake: Arc<(Mutex<Option<Message<T>>>, Condvar)>,
}

impl<T> Sender<T> {
    pub fn send(&self, val: T) {
        *self.wake.0.lock() = Some(Message(val));
        self.wake.1.notify_one();
    }

    pub fn terminate(self) {
        *self.wake.0.lock() = Some(Terminate);
        self.wake.1.notify_one();
    }
}

pub struct Receiver<T> {
    wake: Arc<(Mutex<Option<Message<T>>>, Condvar)>,
}

trait OpaqueOption {
    fn is_some(&self) -> bool;
}

impl<T> OpaqueOption for Option<T> {
    fn is_some(&self) -> bool {
        Option::is_some(self)
    }
}

pub struct Interrupter<'a> {
    mutex: &'a Mutex<dyn OpaqueOption + 'a>,
    is_interrupted: bool,
}

impl<'a> Interrupter<'a> {
    fn new<T>(mutex: &'a Mutex<Option<Message<T>>>) -> Self {
        Self {
            mutex,
            is_interrupted: false,
        }
    }

    pub fn interrupted(&mut self) -> bool {
        if self.is_interrupted {
            true
        } else if self.mutex.lock().is_some() {
            // the Mutex value won't be reset to None until the scope which owns the Interrupter returns
            self.is_interrupted = true;
            true
        } else {
            false
        }
    }
}

struct MultiInterrupterInner<'a> {
    mutex: &'a Mutex<dyn OpaqueOption + Send + 'a>,
    is_interrupted: AtomicBool,
}

#[derive(Clone)]
pub struct MultiInterrupter<'a> {
    inner: Arc<MultiInterrupterInner<'a>>,
}

impl<'a> MultiInterrupter<'a> {
    fn new<T: Send + 'a>(mutex: &'a Mutex<Option<Message<T>>>) -> Self {
        Self {
            inner: Arc::new(MultiInterrupterInner {
                mutex,
                is_interrupted: AtomicBool::new(false),
            }),
        }
    }

    pub fn interrupted(&self) -> bool {
        if self.inner.is_interrupted.load(Ordering::Relaxed) {
            true
        } else if self.inner.mutex.lock().is_some() {
            // the Mutex value won't be reset to None until the scope which owns the Interrupter returns
            self.inner.is_interrupted.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
assert_impl_all!(MultiInterrupter: Send, Sync);

macro_rules! run_function {
    ($name:ident, $interrupter:ident) => {
        pub fn $name(
            self,
            wake_interval: Option<Duration>,
            holdoff: Option<Duration>,
            mut f: impl FnMut(T, $interrupter),
        ) {
            let (mutex, cvar) = &*self.wake;
            'outer: loop {
                let mut val = {
                    // the lock is only held inside this block
                    // outside, it's free to receive updates
                    let mut lock = mutex.lock();
                    loop {
                        match lock.take() {
                            Some(Message(msg)) => break msg,
                            Some(Terminate) => break 'outer,
                            None => {}
                        }

                        if let Some(wake_interval) = wake_interval {
                            cvar.wait_for(&mut lock, wake_interval);
                        } else {
                            cvar.wait(&mut lock);
                        }
                    }
                };

                if let Some(holdoff) = holdoff {
                    loop {
                        thread::sleep(holdoff);

                        match mutex.lock().take() {
                            Some(Message(new_val)) => val = new_val,
                            Some(Terminate) => break 'outer,
                            None => break,
                        }
                    }
                }

                f(val, $interrupter::new(mutex))
            }
        }
    };
}

impl<T> Receiver<T> {
    run_function!(run, Interrupter);
}

impl<T: Send> Receiver<T> {
    run_function!(run_multithreaded, MultiInterrupter);
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let wake = Arc::new((Mutex::new(None), Condvar::new()));
    let sender = Sender { wake: wake.clone() };
    let receiver = Receiver { wake };
    (sender, receiver)
}
