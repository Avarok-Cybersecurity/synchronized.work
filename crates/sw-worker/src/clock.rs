//! Wall-clock time inside the isolate.

use sw_protocol::ports::Clock;

pub struct IsolateClock;

impl Clock for IsolateClock {
    fn now_ms(&self) -> u64 {
        js_sys::Date::now() as u64
    }
}
