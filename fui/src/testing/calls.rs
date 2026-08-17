//! The log of what crossed the boundary, and what a test reads it back as.

use std::cell::RefCell;

/// One thing the Rust side asked the C++ side to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Attach {
        width: i32,
        height: i32,
    },
    Clear,
    Text {
        x: i32,
        y: i32,
        text: String,
        font: i32,
        style: u8,
    },
    Rect {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        black: bool,
    },
    Stroke {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    },
    Line {
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
    },
    Dither {
        light: bool,
    },
    Scrim,
    Clip {
        w: i32,
        h: i32,
    },
    Image {
        w: i32,
        h: i32,
    },
    Icon {
        role: u16,
        size: i32,
    },
    /// `None` where the label crossed as null, meaning "your own label".
    Header {
        title: Option<String>,
        subtitle: Option<String>,
    },
    SubHeader {
        label: String,
        right: Option<String>,
    },
    Hints([Option<String>; 4]),
    Progress {
        current: u32,
        total: u32,
    },
    Slider {
        value: i32,
        max: i32,
    },
    ScrollIndicator {
        content: i32,
        visible: i32,
        offset: i32,
    },
    /// Every cell the C++ side pulled back through the callback.
    List {
        rows: usize,
        selected: i32,
        cells: Vec<[Option<String>; 3]>,
    },
    Popup {
        title: String,
        selected: i32,
        options: Vec<Option<String>>,
    },
    RequestUpdate,
}

thread_local! {
    static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
}

pub(super) fn record(call: Call) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}

/// Forgets everything recorded. Call at the start of each test.
pub fn reset() {
    CALLS.with(|calls| calls.borrow_mut().clear());
}

/// Everything the Rust side sent across, in order.
pub fn calls() -> Vec<Call> {
    CALLS.with(|calls| calls.borrow().clone())
}
