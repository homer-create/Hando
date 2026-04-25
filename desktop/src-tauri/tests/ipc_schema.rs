// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Locks the JSON wire format of every event payload emitted to the frontend.
// Any silent field rename or type change produces a snapshot diff in PR review.

use desktop_lib::encoder::event_sink::*;

#[test]
fn file_done_payload_schema() {
    let p = FileDonePayload { id: "abc-123".into(), src_bytes: 1024, out_bytes: 512 };
    insta::assert_json_snapshot!(p);
}

#[test]
fn file_error_payload_schema() {
    let p = FileErrorPayload { id: "abc-123".into(), msg: "decode failed".into() };
    insta::assert_json_snapshot!(p);
}

#[test]
fn file_skipped_payload_schema() {
    let p = FileSkippedPayload { id: "abc-123".into(), src_bytes: 1024 };
    insta::assert_json_snapshot!(p);
}

#[test]
fn companion_error_payload_schema() {
    let p = CompanionErrorPayload {
        id: "abc-123".into(),
        ext: "webp".into(),
        msg: "ravif failed".into(),
    };
    insta::assert_json_snapshot!(p);
}

#[test]
fn trash_fallback_payload_schema() {
    let p = TrashFallbackPayload {
        id: "abc-123".into(),
        note: "Trash unavailable; original backed up to /tmp/foo.jpg.original".into(),
    };
    insta::assert_json_snapshot!(p);
}

#[test]
fn batch_done_payload_schema() {
    let p = BatchDonePayload { batch_id: "batch-1".into() };
    insta::assert_json_snapshot!(p);
}
