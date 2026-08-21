//! One host-side definition per C symbol, each recording what it was given.

use std::ffi::CStr;

use super::calls::{Call, record};
use crate::raw::CellFn;

/// A C string as Rust sees it. Null becomes `None`, which is the distinction
/// the whole ABI turns on.
///
/// # Safety
/// `ptr` must be null or a valid NUL-terminated string.
unsafe fn borrow(ptr: *const u8) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // Safety: the caller guarantees a valid NUL-terminated string.
    Some(
        unsafe { CStr::from_ptr(ptr.cast()) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// Pulls every cell back through the callback, the way the real shim does.
///
/// # Safety
/// `cell` and `ctx` must be the pair the caller passed in, still valid.
unsafe fn pull(
    cell: CellFn,
    ctx: *mut core::ffi::c_void,
    rows: i32,
    fields: i32,
) -> Vec<Vec<Option<String>>> {
    (0..rows.max(0))
        .map(|index| {
            (0..fields)
                .map(|field| unsafe { borrow(cell(ctx, index, field)) })
                .collect()
        })
        .collect()
}

/// The panel the doubles claim to be. A portrait e-reader.
pub const WIDTH: i32 = 480;
pub const HEIGHT: i32 = 800;

/// Metrics the doubles answer with, indexed by `ThemeMetric`'s own tag values.
///
/// Every value is **distinct**, which matters more than it looks: the tags are
/// positional and not in declaration order (`ListRowGap` is 14, after the
/// slider values), so two metrics answering the same number would let a
/// swapped tag pass unnoticed. They are all non-zero for the same reason a
/// real backend's are — a layout measured against zero collapses silently.
const METRICS: [i32; 17] = [
    8,   // 0  TopPadding
    40,  // 1  HeaderHeight
    12,  // 2  VerticalSpacing
    38,  // 3  ButtonHintsHeight
    16,  // 4  ContentSidePadding
    60,  // 5  ContentTop
    760, // 6  ContentBottom
    44,  // 7  ListRowHeight
    56,  // 8  ListRowHeightWithSubtitle
    6,   // 9  ProgressBarHeight
    48,  // 10 MinTouchSize
    14,  // 11 SliderKnobWidth
    22,  // 12 SliderKnobHeight
    10,  // 13 SliderSideInset
    4,   // 14 ListRowGap
    17,  // 15 SubHeaderHeight
    5,   // 16 SpacingSmall
];

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_attach(_framebuffer: *mut u8, width: i32, height: i32) {
    record(Call::Attach { width, height });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_screen_width() -> i32 {
    WIDTH
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_screen_height() -> i32 {
    HEIGHT
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_clear() {
    record(Call::Clear);
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_text(x: i32, y: i32, text: *const u8, font_id: i32, style: u8) {
    record(Call::Text {
        x,
        y,
        text: unsafe { borrow(text) }.unwrap_or_default(),
        font: font_id,
        style,
    });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_fill_rect(x: i32, y: i32, w: i32, h: i32, black: u8) {
    record(Call::Rect {
        x,
        y,
        w,
        h,
        black: black != 0,
    });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_stroke_rect(x: i32, y: i32, w: i32, h: i32) {
    record(Call::Stroke { x, y, w, h });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_line(x1: i32, y1: i32, x2: i32, y2: i32) {
    record(Call::Line { x1, y1, x2, y2 });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_fill_rect_dither(_x: i32, _y: i32, _w: i32, _h: i32, light: u8) {
    record(Call::Dither { light: light != 0 });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_scrim(_x: i32, _y: i32, _w: i32, _h: i32) {
    record(Call::Scrim);
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_set_clip(_x: i32, _y: i32, w: i32, h: i32) {
    record(Call::Clip { w, h });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_image(_bitmap: *const u8, _x: i32, _y: i32, w: i32, h: i32) {
    record(Call::Image { w, h });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_icon(role: u16, _variant: u8, size: i32, _x: i32, _y: i32) {
    record(Call::Icon { role, size });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_icon_size(_role: u16, _variant: u8, size: i32) -> i32 {
    size
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_font(role: u8) -> i32 {
    // Deliberately not zero: zero means "this build ships no such font", and a
    // double that returned it would make every test silently draw nothing.
    1000 + role as i32
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_text_width(_font_id: i32, text: *const u8, _style: u8) -> i32 {
    unsafe { borrow(text) }.map_or(0, |text| text.chars().count() as i32 * 8)
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_line_height(_font_id: i32) -> i32 {
    17
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_metric(metric: u8) -> i32 {
    METRICS.get(metric as usize).copied().unwrap_or(0)
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_header(title: *const u8, subtitle: *const u8) {
    record(Call::Header {
        title: unsafe { borrow(title) },
        subtitle: unsafe { borrow(subtitle) },
    });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_sub_header(
    _x: i32,
    _y: i32,
    _w: i32,
    _h: i32,
    label: *const u8,
    right: *const u8,
) {
    record(Call::SubHeader {
        label: unsafe { borrow(label) }.unwrap_or_default(),
        right: unsafe { borrow(right) },
    });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_button_hints(
    back: *const u8,
    _back_word: i32,
    confirm: *const u8,
    _confirm_word: i32,
    previous: *const u8,
    _previous_word: i32,
    next: *const u8,
    _next_word: i32,
) {
    record(Call::Hints([
        unsafe { borrow(back) },
        unsafe { borrow(confirm) },
        unsafe { borrow(previous) },
        unsafe { borrow(next) },
    ]));
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_progress_bar(
    _x: i32,
    _y: i32,
    _w: i32,
    _h: i32,
    current: u32,
    total: u32,
) {
    record(Call::Progress { current, total });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_slider(
    _x: i32,
    _y: i32,
    _w: i32,
    _h: i32,
    value: i32,
    max: i32,
    _state: i32,
) {
    record(Call::Slider { value, max });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_scroll_indicator(
    _x: i32,
    _y: i32,
    _w: i32,
    _h: i32,
    content: i32,
    visible: i32,
    offset: i32,
) {
    record(Call::ScrollIndicator {
        content,
        visible,
        offset,
    });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_list(
    _x: i32,
    _y: i32,
    _w: i32,
    _h: i32,
    rows: i32,
    selected: i32,
    cell: CellFn,
    ctx: *mut core::ffi::c_void,
) {
    let pulled = unsafe { pull(cell, ctx, rows, 3) };
    record(Call::List {
        rows: rows.max(0) as usize,
        selected,
        cells: pulled
            .into_iter()
            .map(|row| [row[0].clone(), row[1].clone(), row[2].clone()])
            .collect(),
    });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_draw_option_popup(
    title: *const u8,
    count: i32,
    selected: i32,
    cell: CellFn,
    ctx: *mut core::ffi::c_void,
) {
    let pulled = unsafe { pull(cell, ctx, count, 1) };
    record(Call::Popup {
        title: unsafe { borrow(title) }.unwrap_or_default(),
        selected,
        options: pulled.into_iter().map(|row| row[0].clone()).collect(),
    });
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_option_popup_row_rect(
    _title: *const u8,
    count: i32,
    index: i32,
    out_xywh: *mut i32,
) -> u8 {
    if index < 0 || index >= count || out_xywh.is_null() {
        return 0;
    }
    // A plausible stacked layout, so a test can tell rows apart.
    let values = [16, 200 + index * 40, WIDTH - 32, 40];
    // Safety: the caller passes four writable `i32`s.
    unsafe { core::ptr::copy_nonoverlapping(values.as_ptr(), out_xywh, 4) };
    1
}

#[unsafe(no_mangle)]
extern "C" fn xpui_fui_request_update() {
    record(Call::RequestUpdate);
}
