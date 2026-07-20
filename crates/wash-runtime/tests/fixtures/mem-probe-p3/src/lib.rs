//! Memory-zeroing probe: adversarial fixture for the pooling allocator's
//! slot-reset paths (default madvise vs `*_KEEP_RESIDENT` memset).
//!
//! Every request grows 64 fresh wasm pages (4 MiB), counts nonzero bytes
//! (the spec requires grown memory to be zero — any nonzero byte is another
//! instance's data leaking through slot reuse), then fills the region with
//! 0xAB to poison the pooled slot for whichever instance occupies it next.

mod bindings {
    wit_bindgen::generate!({
        generate_all,
    });
}

use bindings::exports::wasi::http::handler::Guest as Handler;
use bindings::wasi::http::types::{ErrorCode, Fields, Request, Response};

struct Component;

impl Handler for Component {
    async fn handle(_request: Request) -> Result<Response, ErrorCode> {
        const PAGES: usize = 64; // 4 MiB
        let prev = core::arch::wasm32::memory_grow(0, PAGES);
        let body_str = if prev == usize::MAX {
            "{\"error\":\"memory_grow failed\"}".to_string()
        } else {
            let base = (prev * 65536) as *mut u8;
            let len = PAGES * 65536;
            let mut nonzero: u64 = 0;
            unsafe {
                for i in 0..len {
                    if *base.add(i) != 0 {
                        nonzero += 1;
                    }
                }
                core::ptr::write_bytes(base, 0xAB, len);
            }
            format!("{{\"nonzero\":{nonzero}}}")
        };

        let headers = Fields::new();
        let body_bytes = body_str.into_bytes();
        let (mut tx, rx) = bindings::wit_stream::new();
        let (trailers_tx, trailers_rx) = bindings::wit_future::new(|| todo!());
        wit_bindgen::spawn_local(async move {
            tx.write_all(body_bytes).await;
            drop(tx);
            let _ = trailers_tx.write(Ok(None)).await;
        });
        let (response, _result) = Response::new(headers, Some(rx), trailers_rx);
        Ok(response)
    }
}

bindings::export!(Component with_types_in bindings);
