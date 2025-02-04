use rayon::prelude::*;
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
        rx.run(Duration::from_millis(10_000), |_, mut interrupter| {
            // the loop takes 2s
            for _ in 0..100 {
                if interrupter.interrupted() {
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
        rx.run_multithreaded(Duration::from_millis(10_000), |_, interrupter| {
            let interrupter = interrupter.clone();
            (0..100).into_par_iter().for_each(|_| {
                for _ in 0..1000 {
                    if interrupter.interrupted() {
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
