// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Serialize;
use std::sync::Mutex;

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileDonePayload {
    pub id: String,
    pub src_bytes: u64,
    pub out_bytes: u64,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileErrorPayload {
    pub id: String,
    pub msg: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileSkippedPayload {
    pub id: String,
    pub src_bytes: u64,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompanionErrorPayload {
    pub id: String,
    pub ext: String,
    pub msg: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrashFallbackPayload {
    pub id: String,
    pub note: String,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BatchDonePayload {
    pub batch_id: String,
}

pub trait EventSink: Send + Sync {
    fn emit_file_done(&self, p: FileDonePayload);
    fn emit_file_error(&self, p: FileErrorPayload);
    fn emit_file_skipped(&self, p: FileSkippedPayload);
    fn emit_companion_error(&self, p: CompanionErrorPayload);
    fn emit_trash_fallback(&self, p: TrashFallbackPayload);
    fn emit_batch_done(&self, p: BatchDonePayload);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockEvent {
    FileDone(FileDonePayload),
    FileError(FileErrorPayload),
    FileSkipped(FileSkippedPayload),
    CompanionError(CompanionErrorPayload),
    TrashFallback(TrashFallbackPayload),
    BatchDone(BatchDonePayload),
}

#[derive(Default)]
pub struct MockSink {
    events: Mutex<Vec<MockEvent>>,
}

impl MockSink {
    pub fn new() -> Self { Self::default() }
    pub fn events(&self) -> Vec<MockEvent> { self.events.lock().unwrap().clone() }
    pub fn count_by_kind(&self, predicate: impl Fn(&MockEvent) -> bool) -> usize {
        self.events.lock().unwrap().iter().filter(|e| predicate(e)).count()
    }
    fn push(&self, e: MockEvent) { self.events.lock().unwrap().push(e); }
}

impl EventSink for MockSink {
    fn emit_file_done(&self, p: FileDonePayload) { self.push(MockEvent::FileDone(p)); }
    fn emit_file_error(&self, p: FileErrorPayload) { self.push(MockEvent::FileError(p)); }
    fn emit_file_skipped(&self, p: FileSkippedPayload) { self.push(MockEvent::FileSkipped(p)); }
    fn emit_companion_error(&self, p: CompanionErrorPayload) { self.push(MockEvent::CompanionError(p)); }
    fn emit_trash_fallback(&self, p: TrashFallbackPayload) { self.push(MockEvent::TrashFallback(p)); }
    fn emit_batch_done(&self, p: BatchDonePayload) { self.push(MockEvent::BatchDone(p)); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_collects_events_in_order() {
        let sink = MockSink::new();
        sink.emit_file_done(FileDonePayload { id: "a".into(), src_bytes: 100, out_bytes: 50 });
        sink.emit_batch_done(BatchDonePayload { batch_id: "b1".into() });
        let events = sink.events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], MockEvent::FileDone(_)));
        assert!(matches!(events[1], MockEvent::BatchDone(_)));
    }

    #[test]
    fn count_by_kind_filters_correctly() {
        let sink = MockSink::new();
        sink.emit_file_done(FileDonePayload { id: "a".into(), src_bytes: 1, out_bytes: 1 });
        sink.emit_file_done(FileDonePayload { id: "b".into(), src_bytes: 1, out_bytes: 1 });
        sink.emit_file_error(FileErrorPayload { id: "c".into(), msg: "x".into() });
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::FileDone(_))), 2);
        assert_eq!(sink.count_by_kind(|e| matches!(e, MockEvent::FileError(_))), 1);
    }
}
