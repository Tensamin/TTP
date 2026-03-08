use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch;

#[derive(Debug)]
pub struct ConnectionHandle {
    closed: AtomicBool,
    close_tx: watch::Sender<bool>,
    close_rx: watch::Receiver<bool>,
}

impl ConnectionHandle {
    pub fn new() -> Self {
        let (close_tx, close_rx) = watch::channel(false);
        Self {
            closed: AtomicBool::new(false),
            close_tx,
            close_rx,
        }
    }

    pub fn is_open(&self) -> bool {
        !self.closed.load(Ordering::SeqCst)
    }

    pub fn close(&self) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            let _ = self.close_tx.send(true);
        }
    }

    pub async fn closed(&mut self) {
        if !self.is_open() {
            return;
        }
        let _ = self.close_rx.changed().await;
    }

    pub fn subscribe_close(&self) -> watch::Receiver<bool> {
        self.close_rx.clone()
    }
}

impl Default for ConnectionHandle {
    fn default() -> Self {
        Self::new()
    }
}
