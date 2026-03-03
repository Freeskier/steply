use std::sync::mpsc::Receiver;

pub(super) fn drain_receiver<T>(rx: &Receiver<T>) -> Vec<T> {
    let mut out = Vec::new();
    while let Ok(item) = rx.try_recv() {
        out.push(item);
    }
    out
}

pub(super) fn recv_latest<T>(rx: &Receiver<T>) -> Option<T> {
    let mut latest = rx.recv().ok()?;
    while let Ok(next) = rx.try_recv() {
        latest = next;
    }
    Some(latest)
}
