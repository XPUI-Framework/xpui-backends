//! Host doubles for dessen's calls, so the backend runs and is tested off the VM.
//!
//! Off `riscv32` there is no VM, and the backend takes dessen's calls from
//! here instead: each records an [`Op`] and answers from a scripted [`Device`]
//! and a queue of events. Measurements are fixed per font, so a test can
//! predict every coordinate: a glyph is [`glyph_width`] wide and a line is
//! [`line_height`] tall.
//!
//! `xpui`'s host is one per process and tests run on threads, so a test holds
//! [`lock`] for its whole body, then calls [`reset`].

extern crate std;

use std::string::String;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::vec::Vec;

use dessen::{DSN_FONT_BODY, DSN_FONT_SMALL};

use crate::Device;

/// One recorded call, with its arguments in dessen's safe-area coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// `dsn_clear`.
    Clear,
    /// `dsn_fill`: x, y, width, height, colour.
    Fill(i32, i32, i32, i32, i32),
    /// `dsn_stroke`: x, y, width, height.
    Stroke(i32, i32, i32, i32),
    /// `dsn_line`: from x, from y, to x, to y.
    Line(i32, i32, i32, i32),
    /// `dsn_text_box`: x, y, the text, font, flags.
    Text(i32, i32, String, i32, i32),
    /// `dsn_bitmap`: x, y, width, height, the bytes passed, format, colour.
    Bitmap(i32, i32, i32, i32, Vec<u8>, i32, i32),
    /// `dsn_refresh`.
    Refresh,
    /// `dsn_set_clip`: x, y, width, height; a negative width lifts the clip.
    Clip(i32, i32, i32, i32),
    /// `dsn_button_hints`: back, confirm, previous, next.
    Hints([String; 4]),
}

struct State {
    device: Device,
    events: Vec<u32>,
    ops: Vec<Op>,
}

static STATE: Mutex<State> = Mutex::new(State {
    device: Device {
        width: 480,
        height: 800,
        has_touch: true,
        has_buttons: true,
        hint_height: 0,
    },
    events: Vec::new(),
    ops: Vec::new(),
});
static TEST: Mutex<()> = Mutex::new(());

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Serialises tests that use the doubles or install a host.
///
/// Hold it for the whole test; a failed test does not poison it for the next
/// one.
pub fn lock() -> MutexGuard<'static, ()> {
    TEST.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts over: `device` is what the next [`Device::read`] sees, and the
/// recorded calls and queued events are dropped.
pub fn reset(device: Device) {
    let mut state = state();
    state.device = device;
    state.events.clear();
    state.ops.clear();
}

/// Queues events for `dsn_wait_event`, in order.
///
/// Once they run out it answers 0, which ends [`crate::run`].
pub fn events(events: &[u32]) {
    state().events.extend_from_slice(events);
}

/// Takes every call recorded since the last take.
pub fn take_ops() -> Vec<Op> {
    core::mem::take(&mut state().ops)
}

/// The width of one glyph in `font`, as the double measures it.
pub fn glyph_width(font: i32) -> i32 {
    match font {
        DSN_FONT_SMALL => 8,
        DSN_FONT_BODY => 10,
        _ => 14,
    }
}

/// The height of a line in `font`, as the double measures it.
pub fn line_height(font: i32) -> i32 {
    match font {
        DSN_FONT_SMALL => 16,
        DSN_FONT_BODY => 20,
        _ => 28,
    }
}

/// The doubles themselves, with dessen's signatures.
pub(crate) mod calls {
    #![allow(clippy::too_many_arguments)]
    use super::{Op, glyph_width, line_height, state};
    use std::string::String;

    fn record(op: Op) {
        state().ops.push(op);
    }

    pub(crate) fn dsn_screen() -> u32 {
        let device = state().device;
        (device.width as u32) << 16 | device.height as u32 & 0xffff
    }

    pub(crate) fn dsn_device(info: &mut [u8]) -> u32 {
        let device = state().device;
        if info.len() < 20 {
            return 0;
        }
        info[18] = device.has_touch.into();
        info[19] = device.has_buttons.into();
        if info.len() < 24 {
            return 20;
        }
        info[22..24].copy_from_slice(&(device.hint_height as u16).to_le_bytes());
        24
    }

    pub(crate) fn dsn_button_hints(labels: &[u8]) -> i32 {
        let text = String::from_utf8_lossy(labels);
        let mut lines = text.split('\n').map(String::from);
        let hints = core::array::from_fn(|_| lines.next().unwrap_or_default());
        record(Op::Hints(hints));
        0
    }

    pub(crate) fn dsn_wait_event() -> u32 {
        let mut state = state();
        if state.events.is_empty() {
            0
        } else {
            state.events.remove(0)
        }
    }

    pub(crate) fn dsn_refresh() {
        record(Op::Refresh);
    }

    pub(crate) fn dsn_set_clip(x: i32, y: i32, w: i32, h: i32) -> i32 {
        record(Op::Clip(x, y, w, h));
        0
    }

    pub(crate) fn dsn_clear() {
        record(Op::Clear);
    }

    pub(crate) fn dsn_fill(x: i32, y: i32, w: i32, h: i32, color: i32) {
        record(Op::Fill(x, y, w, h, color));
    }

    pub(crate) fn dsn_stroke(x: i32, y: i32, w: i32, h: i32, _: i32, _: i32, _: i32, _: i32) {
        record(Op::Stroke(x, y, w, h));
    }

    pub(crate) fn dsn_line(x0: i32, y0: i32, x1: i32, y1: i32, _width: i32, _color: i32) {
        record(Op::Line(x0, y0, x1, y1));
    }

    pub(crate) fn dsn_text_box(
        x: i32,
        y: i32,
        _w: i32,
        _h: i32,
        s: &[u8],
        font: i32,
        _align: i32,
        _max_lines: i32,
        flags: i32,
    ) -> i32 {
        record(Op::Text(
            x,
            y,
            String::from_utf8_lossy(s).into_owned(),
            font,
            flags,
        ));
        0
    }

    pub(crate) fn dsn_measure_text(s: &[u8], font: i32, _flags: i32) -> u32 {
        let chars = String::from_utf8_lossy(s).chars().count() as i32;
        ((chars * glyph_width(font)) as u32) << 16 | line_height(font) as u32
    }

    pub(crate) fn dsn_line_height(font: i32) -> i32 {
        line_height(font)
    }

    pub(crate) fn dsn_bitmap(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        data: &[u8],
        _bw: i32,
        _bh: i32,
        format: i32,
        _mode: i32,
        color: i32,
        _rotation: i32,
    ) -> i32 {
        record(Op::Bitmap(x, y, w, h, data.to_vec(), format, color));
        0
    }
}
