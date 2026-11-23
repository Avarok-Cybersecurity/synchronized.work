//! A host measure that counts its calls (and can refuse them), stamped with a test clock.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use sw_app_core::{HostText, TextMeasure};

use super::FixedAdvance;

#[derive(Debug, Clone, Default)]
pub struct Counted {
    /// When each measure was asked, by the test's clock.
    pub at: Rc<RefCell<Vec<f64>>>,
    /// The test's clock (frame time it has fed the App).
    pub now: Rc<Cell<f64>>,
    pub refuse: Rc<Cell<bool>>,
}

impl Counted {
    pub fn calls(&self) -> usize {
        self.at.borrow().len()
    }
}

impl HostText for Counted {
    fn measure(&self, text: &str, px: f32) -> Option<f32> {
        self.at.borrow_mut().push(self.now.get());
        (!self.refuse.get()).then(|| FixedAdvance.width(text, px))
    }
}
