#![feature(stdarch_wasm_atomic_wait)]

//! Browser queue regression from Zed 5c9efb75cf, retained at 635026fabc.
//! The Node driver exercises the real browser main thread and joins every worker.

use open_gpui::{Priority, PriorityQueueReceiver, PriorityQueueSender};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use wasm_bindgen::prelude::*;

fn main() {}

/// Exercises both directions of a queue shared by the browser and real workers.
#[wasm_bindgen]
pub struct QueueStress {
    sender: PriorityQueueSender<Option<usize>>,
    receiver: PriorityQueueReceiver<usize>,
    ready: Arc<AtomicUsize>,
    stopped: Arc<AtomicUsize>,
    workers: Vec<wasm_thread::JoinHandle<()>>,
    seen: Vec<bool>,
    sent: usize,
    received: usize,
}

#[wasm_bindgen]
impl QueueStress {
    #[wasm_bindgen(constructor)]
    pub fn new(worker_count: usize, item_count: usize) -> Self {
        assert!(worker_count > 0 && item_count > 0);
        let (sender, receiver) = PriorityQueueReceiver::<Option<usize>>::new();
        let (reply_sender, reply_receiver) = PriorityQueueReceiver::new();
        let reply_sender = Arc::new(reply_sender);
        let ready = Arc::new(AtomicUsize::new(0));
        let stopped = Arc::new(AtomicUsize::new(0));
        let workers = (0..worker_count)
            .map(|_| {
                let mut receiver = receiver.clone();
                let reply_sender = reply_sender.clone();
                let ready = ready.clone();
                let stopped = stopped.clone();
                wasm_thread::spawn(move || {
                    ready.fetch_add(1, Ordering::Release);
                    while let Some(item) = receiver.pop().expect("receive work") {
                        reply_sender.send(priority(item), item).expect("send reply");
                    }
                    stopped.fetch_add(1, Ordering::Release);
                })
            })
            .collect();
        Self {
            sender,
            receiver: reply_receiver,
            ready,
            stopped,
            workers,
            seen: vec![false; item_count],
            sent: 0,
            received: 0,
        }
    }

    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Acquire) == self.workers.len()
    }

    pub fn send_batch(&mut self, count: usize) {
        let end = (self.sent + count).min(self.seen.len());
        for item in self.sent..end {
            self.sender
                .spin_send(priority(item), Some(item))
                .expect("send work from browser main thread");
        }
        self.sent = end;
    }

    pub fn drain(&mut self) -> usize {
        while let Some(item) = self
            .receiver
            .spin_try_pop()
            .expect("receive reply on browser main thread")
        {
            let seen = self.seen.get_mut(item).expect("unexpected reply");
            assert!(!*seen, "duplicate reply");
            *seen = true;
            self.received += 1;
        }
        self.received
    }

    pub fn stop(&mut self) {
        assert_eq!(self.sent, self.seen.len());
        assert_eq!(self.received, self.sent);
        assert!(self.seen.iter().all(|seen| *seen));
        for _ in &self.workers {
            self.sender
                .spin_send(Priority::Low, None)
                .expect("stop worker");
        }
    }

    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire) == self.workers.len()
    }

    /// Waits for stopped workers' thread epilogues without blocking the browser main thread.
    pub async fn join_workers(&mut self) -> usize {
        assert!(self.stopped(), "all workers must stop before joining");
        let workers = std::mem::take(&mut self.workers);
        let worker_count = workers.len();
        for worker in workers {
            worker.join_async().await.expect("join worker");
        }
        worker_count
    }
}

fn priority(item: usize) -> Priority {
    match item % 3 {
        0 => Priority::High,
        1 => Priority::Medium,
        _ => Priority::Low,
    }
}

/// Negative control: a browser-main Wasm atomic wait must trap, even with zero timeout.
#[wasm_bindgen]
pub fn forbidden_main_thread_wait() {
    let value = std::sync::atomic::AtomicI32::new(0);
    // SAFETY: the pointer is aligned and valid for the duration of this call.
    // The browser must reject the wait on its main thread.
    unsafe {
        std::arch::wasm32::memory_atomic_wait32(value.as_ptr(), 0, 0);
    }
}
