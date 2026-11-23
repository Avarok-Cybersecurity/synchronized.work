//! Text widths, each `(text, px)` asked of the host once. Every width the App uses — a
//! screen's wrapping, a button's size, a tree label's truncation — goes through one
//! [`MeasureCache`], so after the first frame of a screen a redraw makes no host call at all.
//!
//! The widths are CSS pixels of `px`-sized text in the theme's one font. Neither can change
//! during an instance's life, and a `Resize` that changes the device pixel ratio does not
//! change them either: the host scales its context by `dpr` once and `measureText` reports
//! widths in the untransformed (CSS pixel) space. So nothing ever invalidates an entry; the
//! least recently used one is forgotten when the cache is full.
//!
//! Host calls are budgeted ([`crate::pace`]): before a frame is built the App grants the cache
//! an allowance of host measures; a width it may not ask for is estimated for that build only
//! and the build is marked starved, so the App does not draw it and builds again later.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use crate::painter::TextMeasure;

/// The host's `canvas.MeasureText`: the width, or `None` when the host refused.
pub trait HostText {
    fn measure(&self, text: &str, px: f32) -> Option<f32>;
}

/// A width source that cannot refuse (a fixed advance, a font table) is a host that never does.
impl<T: TextMeasure> HostText for T {
    fn measure(&self, text: &str, px: f32) -> Option<f32> {
        Some(self.width(text, px))
    }
}

/// Explicit (PCND): how many widths are remembered, and the stand-in for one the host refused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasureParams {
    /// Widths remembered; when full, the least recently used is forgotten.
    pub capacity: usize,
    /// A width not measured (refused, or over the allowance) is taken to be this many `px` per
    /// character. It is never remembered.
    pub estimate_em: f32,
}

/// What one build asked of the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asked {
    /// Host measure calls made.
    pub calls: u32,
    /// A width was needed past the allowance: the build used an estimate and must not be drawn.
    pub starved: bool,
}

type Key = (String, u32);

#[derive(Debug, Default)]
struct Memo {
    widths: BTreeMap<Key, (f32, u64)>,
    by_use: BTreeMap<u64, Key>,
    clock: u64,
}

impl Memo {
    fn get(&mut self, key: &Key) -> Option<f32> {
        self.clock += 1;
        let clock = self.clock;
        let (width, used) = self.widths.get_mut(key)?;
        self.by_use.remove(used);
        *used = clock;
        self.by_use.insert(clock, key.clone());
        Some(*width)
    }

    fn put(&mut self, key: Key, width: f32, capacity: usize) {
        if capacity == 0 {
            return;
        }
        while self.widths.len() >= capacity {
            let Some((_, oldest)) = self.by_use.pop_first() else {
                break;
            };
            self.widths.remove(&oldest);
        }
        self.clock += 1;
        self.by_use.insert(self.clock, key.clone());
        self.widths.insert(key, (width, self.clock));
    }
}

#[derive(Debug)]
pub struct MeasureCache<M> {
    host: M,
    params: MeasureParams,
    memo: RefCell<Memo>,
    allowance: Cell<u32>,
    calls: Cell<u32>,
    starved: Cell<bool>,
}

impl<M: HostText> MeasureCache<M> {
    pub fn new(host: M, params: MeasureParams) -> Self {
        Self {
            host,
            params,
            memo: RefCell::new(Memo::default()),
            allowance: Cell::new(0),
            calls: Cell::new(0),
            starved: Cell::new(false),
        }
    }

    /// Starts a build that may make at most `calls` host measures.
    pub fn allow(&self, calls: u32) {
        self.allowance.set(calls);
        self.calls.set(0);
        self.starved.set(false);
    }

    /// Ends the build: what it asked of the host. No host measure is allowed until the next.
    pub fn settle(&self) -> Asked {
        self.allowance.set(0);
        Asked {
            calls: self.calls.replace(0),
            starved: self.starved.replace(false),
        }
    }

    /// How many widths are remembered.
    pub fn len(&self) -> usize {
        self.memo.borrow().widths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn estimate(&self, text: &str, px: f32) -> f32 {
        text.chars().count() as f32 * px * self.params.estimate_em
    }
}

impl<M: HostText> TextMeasure for MeasureCache<M> {
    fn width(&self, text: &str, px: f32) -> f32 {
        let key = (text.to_owned(), px.to_bits());
        if let Some(width) = self.memo.borrow_mut().get(&key) {
            return width;
        }
        let allowance = self.allowance.get();
        if allowance == 0 {
            self.starved.set(true);
            return self.estimate(text, px);
        }
        self.allowance.set(allowance - 1);
        self.calls.set(self.calls.get() + 1);
        match self.host.measure(text, px) {
            Some(width) => {
                self.memo.borrow_mut().put(key, width, self.params.capacity);
                width
            }
            None => self.estimate(text, px),
        }
    }
}
