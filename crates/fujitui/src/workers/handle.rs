use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread::JoinHandle,
};

use crossbeam_channel::Sender;

pub struct WorkerHandle<C> {
    command_tx: Option<Sender<C>>,
    inflight: Arc<AtomicUsize>,
    join: Option<JoinHandle<()>>,
}

impl<C> WorkerHandle<C> {
    pub const fn new(
        command_tx: Sender<C>,
        inflight: Arc<AtomicUsize>,
        join: JoinHandle<()>,
    ) -> Self {
        Self {
            command_tx: Some(command_tx),
            inflight,
            join: Some(join),
        }
    }

    #[allow(dead_code)]
    pub fn send(&self, cmd: C) {
        if let Some(tx) = self.command_tx.as_ref() {
            self.inflight.fetch_add(1, Ordering::Relaxed);
            if tx.send(cmd).is_err() {
                self.inflight.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }

    pub fn is_busy(&self) -> bool {
        self.inflight.load(Ordering::Relaxed) > 0
    }
}

impl<C> Drop for WorkerHandle<C> {
    fn drop(&mut self) {
        drop(self.command_tx.take());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
