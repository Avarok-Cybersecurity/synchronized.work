//! The `net.host` fallback's request: `HostFetchOpen`, then `NetStreamRead` until the body ends,
//! then `NetStreamClose`, and the whole response to the core as one `HttpResponse`.

use citadel_app_sdk::net::{self, NetStream};
use sw_app_core::Input;
use sw_app_core::fallback::HttpGet;

use crate::app::{SwApp, warn};
use crate::config::{STREAM_MAX_BODY_BYTES, STREAM_READ_BYTES};
use crate::map::{self, StreamStep, reason_of};

pub(crate) fn open(app: &mut SwApp, get: HttpGet) {
    let request = match map::http_request_of(get) {
        Ok(r) => r,
        Err(e) => {
            return app.feed(Input::HttpFailed {
                reason: format!("{e:?}"),
            });
        }
    };
    net::host_fetch_stream(request).then(|app: &mut SwApp, r| match r {
        Ok(stream) => read(stream, Vec::new()),
        Err(e) => app.feed(Input::HttpFailed {
            reason: reason_of(&e),
        }),
    });
}

fn read(stream: NetStream, mut body: Vec<u8>) {
    let next = stream.read(STREAM_READ_BYTES);
    next.then(move |app: &mut SwApp, r| {
        let step = r.map(|chunk| map::read_step(&mut body, chunk, STREAM_MAX_BODY_BYTES));
        match step {
            Ok(StreamStep::More) => read(stream, body),
            Ok(StreamStep::End) => {
                let status = stream.head.status;
                close(stream);
                app.feed(Input::HttpResponse { status, body });
            }
            Ok(StreamStep::TooLarge) => {
                close(stream);
                app.feed(Input::HttpFailed {
                    reason: format!("the response passed {STREAM_MAX_BODY_BYTES} bytes"),
                });
            }
            Err(e) => {
                close(stream);
                app.feed(Input::HttpFailed {
                    reason: reason_of(&e),
                });
            }
        }
    });
}

fn close(stream: NetStream) {
    stream
        .close()
        .then(|_: &mut SwApp, r| r.err().iter().for_each(|e| warn("stream close", e)));
}
