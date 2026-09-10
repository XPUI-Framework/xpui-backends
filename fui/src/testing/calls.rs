//! The log of what crossed the boundary, and what a test reads it back as.

use std::cell::RefCell;

/// One thing the Rust side asked the C++ side to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// The shim was pointed at a framebuffer.
    Attach {
        /// Panel width in pixels.
        width: i32,
        /// Panel height in pixels.
        height: i32,
    },
    /// The whole panel cleared to background.
    Clear,
    /// A line of text.
    Text {
        /// Its top-left corner's x.
        x: i32,
        /// Its top-left corner's y.
        y: i32,
        /// The string as it crossed.
        text: String,
        /// The font id the Rust side asked for.
        font: i32,
        /// The style tag, `FontStyle`'s discriminant.
        style: u8,
    },
    /// A filled rectangle.
    Rect {
        /// Left edge.
        x: i32,
        /// Top edge.
        y: i32,
        /// Width.
        w: i32,
        /// Height.
        h: i32,
        /// Ink, or background.
        black: bool,
    },
    /// An outlined rectangle.
    Stroke {
        /// Left edge.
        x: i32,
        /// Top edge.
        y: i32,
        /// Width.
        w: i32,
        /// Height.
        h: i32,
    },
    /// A one-pixel line.
    Line {
        /// One end's x.
        x1: i32,
        /// One end's y.
        y1: i32,
        /// The other end's x.
        x2: i32,
        /// The other end's y.
        y2: i32,
    },
    /// A dithered fill; the rect is not recorded.
    Dither {
        /// Which checkerboard parity took ink.
        light: bool,
    },
    /// A scrim over a rect.
    Scrim,
    /// A clip set, or lifted when either dimension is zero.
    Clip {
        /// The clip's width.
        w: i32,
        /// The clip's height.
        h: i32,
    },
    /// A 1-bit bitmap drawn.
    Image {
        /// Its width in pixels.
        w: i32,
        /// Its height in pixels.
        h: i32,
    },
    /// An icon drawn.
    Icon {
        /// The icon's role, as the host numbers them.
        role: u16,
        /// The edge length asked for.
        size: i32,
    },
    /// The header band; `None` where a part crossed as null.
    Header {
        /// The title, or `None` for none.
        title: Option<String>,
        /// The subtitle, or `None` for none.
        subtitle: Option<String>,
    },
    /// A section heading.
    SubHeader {
        /// The heading.
        label: String,
        /// A right-aligned value, or `None` for none.
        right: Option<String>,
    },
    /// The four hint slots in meaning order; `None` is the host's own word.
    Hints([Option<String>; 4]),
    /// A progress bar.
    Progress {
        /// Progress so far, out of `total`.
        current: u32,
        /// The whole.
        total: u32,
    },
    /// A slider.
    Slider {
        /// Where the knob sits, out of `max`.
        value: i32,
        /// The top of the range.
        max: i32,
    },
    /// A scroll indicator.
    ScrollIndicator {
        /// How tall the content is.
        content: i32,
        /// How much of the content the window shows.
        visible: i32,
        /// How far down the window sits.
        offset: i32,
    },
    /// Every cell the C++ side pulled back through the callback.
    List {
        /// How many rows were asked for.
        rows: usize,
        /// The selected row, or -1 for none.
        selected: i32,
        /// Title, subtitle and value per row; `None` is a field the row omitted.
        cells: Vec<[Option<String>; 3]>,
    },
    /// An option dialog.
    Popup {
        /// The dialog's title.
        title: String,
        /// The highlighted option.
        selected: i32,
        /// Every option pulled back through the callback; `None` is a null.
        options: Vec<Option<String>>,
    },
    /// The panel was asked to show what has been drawn.
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
