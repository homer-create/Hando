// Copyright (C) 2025 謝昇運 (homershie) <homerxworkshop@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Integration tests for the encoder facade + batch lifecycle.
// Uses MockSink so no Tauri runtime is needed.

use desktop_lib::batch::BatchState;
use desktop_lib::encoder::event_sink::{MockEvent, MockSink};
use desktop_lib::encoder::{encode, EncodeOpts, EncodeOutcome, EncodeRequest, ImageExt};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn opts() -> EncodeOpts {
    EncodeOpts {
        jpeg_quality: 80,
        png_quality: 80,
        webp_quality: 80,
        avif_quality: 60,
        emit_webp: false,
        emit_avif: false,
        ..EncodeOpts::default()
    }
}

#[test]
fn batch_lifecycle_emits_one_batch_done_for_three_files() {
    let state = Arc::new(BatchState::default());
    let sink = Arc::new(MockSink::new());

    state.start("batch-A", 3);

    let inputs = vec![
        (fixture("landscape.jpg"), ImageExt::Jpeg),
        (fixture("transparent.png"), ImageExt::Png),
        (fixture("screenshot.png"), ImageExt::Png),
    ];

    let mut handles = vec![];
    for (path, ext) in inputs {
        let s = state.clone();
        let snk = sink.clone();
        let o = opts();
        handles.push(thread::spawn(move || {
            // Run the encode (we don't do the actual file replacement in this test)
            let _outcome = encode(EncodeRequest { src_path: &path, ext, opts: &o, progress_cb: None });
            // Tick regardless of outcome (mirrors what commands.rs does)
            s.tick("batch-A", &*snk);
        }));
    }
    for h in handles { h.join().unwrap(); }

    let bd = sink.count_by_kind(|e| matches!(e, MockEvent::BatchDone(_)));
    assert_eq!(bd, 1, "expected exactly one batch-done event, got {bd}");
}

#[test]
fn corrupt_input_encode_returns_decode_error() {
    let result = encode(EncodeRequest {
        src_path: &fixture("corrupt.jpg"),
        ext: ImageExt::Jpeg,
        opts: &opts(),
        progress_cb: None,
    });
    assert!(
        matches!(result, Err(desktop_lib::encoder::EncodeError::Decode(_))),
        "expected Decode error for corrupt input, got: {:?}", result
    );
}

#[test]
fn webp_and_avif_companions_emitted_when_enabled() {
    let mut o = opts();
    o.emit_webp = true;
    o.emit_avif = true;

    let outcome = encode(EncodeRequest {
        src_path: &fixture("landscape.jpg"),
        ext: ImageExt::Jpeg,
        opts: &o,
        progress_cb: None,
    }).unwrap();

    if let EncodeOutcome::Encoded(r) = outcome {
        assert_eq!(r.companions.len(), 2, "expected 2 companions (webp + avif)");
        let exts: Vec<_> = r.companions.iter().map(|c| c.ext).collect();
        assert!(exts.contains(&ImageExt::Webp), "should have WebP companion");
        assert!(exts.contains(&ImageExt::Avif), "should have AVIF companion");
        assert!(r.companion_errors.is_empty());
    } else {
        panic!("expected Encoded outcome");
    }
}
