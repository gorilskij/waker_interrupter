use parking_lot::Mutex;
use rayon::prelude::*;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use waker_interrupter::channel;

fn sleep(millis: u64) {
    thread::sleep(Duration::from_millis(millis));
}

#[test]
fn integration_test() {
    let (tx, rx) = channel();

    let handle = thread::spawn(move || {
        rx.run(None, None, |_, mut int| {
            // the loop takes 2s
            for _ in 0..100 {
                if int.interrupted() {
                    return;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
    });

    let start = Instant::now();

    // t = 0
    tx.send(0);
    sleep(100);
    // t = 100
    tx.send(0);
    sleep(100);
    // t = 200
    tx.send(0);
    sleep(100);
    // t = 300
    tx.terminate();
    handle.join().unwrap();
    // t = ~300

    let elapsed = start.elapsed().as_millis();
    let error = elapsed.abs_diff(300);
    assert!(error < 50, "{}", error);
}

#[test]
fn multithreaded_test() {
    let (tx, rx) = channel();

    let handle = thread::spawn(move || {
        rx.run_multithreaded(None, None, |_, int| {
            let int = int.clone();
            (0..100).into_par_iter().for_each(|_| {
                for _ in 0..1000 {
                    if int.interrupted() {
                        return;
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            });
        });
    });

    let start = Instant::now();

    // t = 0
    tx.send(0);
    sleep(100);
    // t = 100
    tx.send(0);
    sleep(100);
    // t = 200
    tx.send(0);
    sleep(100);
    // t = 300
    tx.terminate();
    handle.join().unwrap();
    // t = ~300

    let elapsed = start.elapsed().as_millis();
    let error = elapsed.abs_diff(300);
    assert!(error < 50, "{}", error);
}

#[test]
fn holdoff_test() {
    let (tx, rx) = channel();

    let values = Arc::new(Mutex::new(vec![]));
    let values_clone = values.clone();

    let handle = thread::spawn(move || {
        rx.run(None, Some(Duration::from_millis(100)), |val, _int| {
            values_clone.lock().push(val);
        });
    });

    tx.send(0);
    sleep(50);

    tx.send(1);
    sleep(200);

    tx.send(2);
    sleep(50);

    tx.terminate();
    handle.join().unwrap();

    assert_eq!(&*values.lock(), &[1]);
}

#[test]
fn no_holdoff_test() {
    let (tx, rx) = channel();

    let values = Arc::new(Mutex::new(vec![]));
    let values_clone = values.clone();

    let handle = thread::spawn(move || {
        rx.run(None, None, |val, _int| {
            values_clone.lock().push(val);
        });
    });

    tx.send(0);
    sleep(50);

    tx.send(1);
    sleep(200);

    tx.send(2);
    sleep(50);

    tx.terminate();
    handle.join().unwrap();

    assert_eq!(&*values.lock(), &[0, 1, 2]);
}
