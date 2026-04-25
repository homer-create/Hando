// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::encoder::event_sink::{BatchDonePayload, EventSink};
use crate::trash::Disposal;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct Batch {
    pub id: String,
    pub disposals: Vec<Disposal>,
    pub expected: usize,
}

struct BatchInner {
    id: String,
    expected: usize,
    completed: AtomicUsize,
    disposals: Mutex<Vec<Disposal>>,
}

#[derive(Default)]
pub struct BatchState {
    inner: Mutex<HashMap<String, BatchInner>>,
    last_complete: Mutex<Option<Batch>>,
}

impl BatchState {
    pub fn start(&self, id: &str, expected: usize) {
        let mut cur = self.inner.lock().unwrap();
        cur.insert(id.to_string(), BatchInner {
            id: id.to_string(),
            expected,
            completed: AtomicUsize::new(0),
            disposals: Mutex::new(vec![]),
        });
    }

    pub fn record_disposal(&self, batch_id: &str, disposal: Disposal) {
        let cur = self.inner.lock().unwrap();
        if let Some(b) = cur.get(batch_id) {
            b.disposals.lock().unwrap().push(disposal);
        }
    }

    pub fn record_companion_paths(&self, batch_id: &str, paths: Vec<PathBuf>) {
        if paths.is_empty() { return; }
        let cur = self.inner.lock().unwrap();
        if let Some(b) = cur.get(batch_id) {
            let mut disposals = b.disposals.lock().unwrap();
            if let Some(d) = disposals.last_mut() {
                d.companion_paths.extend(paths);
            }
        }
    }

    /// Increment the completion counter. If this was the last expected file,
    /// emit `batch-done` and move the batch to `last_complete`.
    pub fn tick(&self, batch_id: &str, sink: &dyn EventSink) {
        let should_complete = {
            let cur = self.inner.lock().unwrap();
            match cur.get(batch_id) {
                Some(b) => {
                    let prev = b.completed.fetch_add(1, Ordering::SeqCst);
                    prev + 1 == b.expected
                }
                None => false,
            }
        };
        if should_complete {
            sink.emit_batch_done(BatchDonePayload { batch_id: batch_id.to_string() });
            self.complete(batch_id);
        }
    }

    fn complete(&self, batch_id: &str) {
        let mut cur = self.inner.lock().unwrap();
        if let Some(inner) = cur.remove(batch_id) {
            let batch = Batch {
                id: inner.id,
                disposals: inner.disposals.into_inner().unwrap(),
                expected: inner.expected,
            };
            *self.last_complete.lock().unwrap() = Some(batch);
        }
    }

    pub fn take_last(&self) -> Option<Batch> {
        self.last_complete.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::event_sink::{MockEvent, MockSink};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn tick_emits_batch_done_when_last_completes() {
        let state = BatchState::default();
        let sink = MockSink::new();
        state.start("b1", 3);

        state.tick("b1", &sink);
        state.tick("b1", &sink);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 0);

        state.tick("b1", &sink);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 1);
    }

    #[test]
    fn tick_is_thread_safe_under_concurrent_load() {
        let state = Arc::new(BatchState::default());
        let sink = Arc::new(MockSink::new());
        state.start("b1", 100);

        let mut handles = vec![];
        for _ in 0..100 {
            let s = state.clone();
            let snk = sink.clone();
            handles.push(thread::spawn(move || s.tick("b1", &*snk)));
        }
        for h in handles { h.join().unwrap(); }

        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_))), 1);
    }
}
