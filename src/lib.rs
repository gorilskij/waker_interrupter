use parking_lot::{Condvar, Mutex};
use std::sync::Arc;
use std::time::Duration;
use Message::*;

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

pub struct Interrupter<'a, T> {
    mutex: &'a Mutex<Option<Message<T>>>,
    is_interrupted: bool,
}

impl<'a, T> Interrupter<'a, T> {
    fn new(mutex: &'a Mutex<Option<Message<T>>>) -> Self {
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

impl<T> Receiver<T> {
    pub fn run(self, wake_interval: Duration, mut f: impl FnMut(T, Interrupter<T>)) {
        let (mutex, cvar) = &*self.wake;
        loop {
            let msg = {
                // the lock is only held inside this block
                // outside, it's free to receive updates
                let mut lock = mutex.lock();
                loop {
                    if let Some(msg) = lock.take() {
                        break msg;
                    }
                    cvar.wait_for(&mut lock, wake_interval);
                }
            };

            match msg {
                Message(val) => f(val, Interrupter::new(mutex)),
                Terminate => break,
            }
        }
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let wake = Arc::new((Mutex::new(None), Condvar::new()));
    let sender = Sender { wake: wake.clone() };
    let receiver = Receiver { wake };
    (sender, receiver)
}
