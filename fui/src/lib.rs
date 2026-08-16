//! An [`xpui`] backend that draws through **FreeInkUI**.
//!
//! FreeInkUI is a header-only C++ component library for e-ink firmware. It
//! already knows what a list row, a dialog and a slider look like, so a screen
//! written against `xpui` and one written in C++ against FreeInkUI come out as
//! the same pixels — which is the whole reason to sit on it rather than beside
//! it.
//!
//! # What ships here
//!
//! Two halves that have to stay in step:
//!
//! | | |
//! |---|---|
//! | `src/` | the Rust `Host` implementation, over a C ABI ([`raw`]) |
//! | `cpp/` | that ABI implemented against FreeInkUI's own `DrawTarget` |
//!
//! The C++ half is deliberately **not** written against any particular
//! firmware's renderer. It binds to `freeink::ui::DisplayTarget`, which is
//! dependency-free and takes a plain 1-bit framebuffer, so any project that
//! links the FreeInk SDK can add these two files and be done. A firmware with
//! its own themed renderer can substitute its own implementation of the same
//! ABI instead.
//!
//! # Wiring it up
//!
//! ```rust,ignore
//! // Once, with the panel's framebuffer.
//! unsafe { xpui_fui::attach(framebuffer.as_mut_ptr(), 480, 800) };
//! unsafe { xpui::host::install(xpui_fui::backend()) };
//! ```
//!
//! Add `cpp/xpui_fui.cpp` to the firmware's build and put FreeInkUI's include
//! directory on its path. Input and the clock are **not** here: they are
//! platform concerns, and a firmware supplies them by implementing
//! [`Platform`].

#![cfg_attr(target_os = "none", no_std)]

extern crate alloc;

use core::ffi::c_void;

use xpui::host::{
    Canvas, Chrome, Clock, FontId, FontRole, FontStyle, Hint, IconRef, InputSource, RowField,
    TextMetrics, ThemeMetric,
};
use xpui::{Button, Point, Rect, Size, SwipeDir};

mod cells;
pub mod raw;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

use cells::{Cells, cell_trampoline};

/// Buttons, gestures and the clock, which FreeInkUI has no opinion about.
///
/// A drawing library cannot tell you whether a button was pressed. Whatever is
/// driving the panel already knows — a firmware's input manager, a GPIO poll —
/// so it implements this and the backend forwards to it.
pub trait Platform: Sync {
    fn millis(&self) -> u32;

    fn was_pressed(&self, button: Button) -> bool;
    fn is_pressed(&self, button: Button) -> bool;
    fn was_released(&self, button: Button) -> bool;

    fn has_touch(&self) -> bool {
        false
    }
    fn tap(&self) -> Option<Point> {
        None
    }
    fn touch_held(&self) -> Option<Point> {
        None
    }
    fn touch_released(&self) -> bool {
        false
    }
    fn swipe(&self) -> SwipeDir {
        SwipeDir::None
    }
    fn was_back_gesture(&self) -> bool {
        false
    }
    fn was_home_gesture(&self) -> bool {
        false
    }
    /// See [`InputSource::swipe_moves_selection`].
    fn swipe_moves_selection(&self) -> bool {
        false
    }
}

/// A platform with no input at all, for a panel that only ever displays.
pub struct NoInput;

impl Platform for NoInput {
    fn millis(&self) -> u32 {
        0
    }
    fn was_pressed(&self, _button: Button) -> bool {
        false
    }
    fn is_pressed(&self, _button: Button) -> bool {
        false
    }
    fn was_released(&self, _button: Button) -> bool {
        false
    }
}

/// The backend. Stateless: everything it needs lives on the C++ side, bound
/// once by [`attach`].
pub struct Backend<P: Platform + 'static> {
    platform: &'static P,
}

impl<P: Platform + 'static> Backend<P> {
    pub const fn new(platform: &'static P) -> Self {
        Backend { platform }
    }
}

/// Points the C++ side at the panel's framebuffer.
///
/// # Safety
/// `framebuffer` must be writable for `(width + 7) / 8 * height` bytes and
/// must outlive every subsequent draw. Call before installing the backend.
pub unsafe fn attach(framebuffer: *mut u8, width: i32, height: i32) {
    unsafe { raw::xpui_fui_attach(framebuffer, width, height) }
}

/// Borrows a NUL-terminated C string the C++ side owns, as bytes.
fn as_c(text: &str) -> alloc::ffi::CString {
    alloc::ffi::CString::new(text).unwrap_or_default()
}

/// The pointer for an optional label: null means "you decide".
fn optional(text: Option<&str>) -> Option<alloc::ffi::CString> {
    text.map(as_c)
}

fn ptr_of(text: &Option<alloc::ffi::CString>) -> *const u8 {
    text.as_ref()
        .map_or(core::ptr::null(), |text| text.as_ptr().cast())
}

impl<P: Platform> Canvas for Backend<P> {
    fn screen_size(&self) -> Size {
        Size::new(raw::xpui_fui_screen_width(), raw::xpui_fui_screen_height())
    }

    fn clear(&self) {
        raw::xpui_fui_clear();
    }

    fn draw_text(&self, origin: Point, text: &str, font: FontId, style: FontStyle) {
        // A font this build compiled out measures zero and must draw nothing,
        // rather than painting at some arbitrary substitute size.
        if !font.is_available() {
            return;
        }
        let text = as_c(text);
        unsafe {
            raw::xpui_fui_draw_text(
                origin.x,
                origin.y,
                text.as_ptr().cast(),
                font.0,
                style as u8,
            )
        }
    }

    fn fill_rect(&self, rect: Rect, black: bool) {
        raw::xpui_fui_fill_rect(
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            u8::from(black),
        );
    }

    fn stroke_rect(&self, rect: Rect) {
        raw::xpui_fui_stroke_rect(rect.x(), rect.y(), rect.width(), rect.height());
    }

    fn draw_line(&self, from: Point, to: Point) {
        raw::xpui_fui_draw_line(from.x, from.y, to.x, to.y);
    }

    fn fill_rect_dither(&self, rect: Rect, light: bool) {
        raw::xpui_fui_fill_rect_dither(
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            u8::from(light),
        );
    }

    fn scrim(&self, rect: Rect) {
        raw::xpui_fui_scrim(rect.x(), rect.y(), rect.width(), rect.height());
    }

    fn set_clip(&self, rect: Option<Rect>) {
        // A zero-sized rect is how the ABI spells "no clip", so `None` becomes
        // one rather than needing a second symbol.
        let rect = rect.unwrap_or(Rect::new(0, 0, 0, 0));
        raw::xpui_fui_set_clip(rect.x(), rect.y(), rect.width(), rect.height());
    }

    fn draw_image(&self, origin: Point, data: &[u8], size: Size) {
        if data.is_empty() || size.width <= 0 || size.height <= 0 {
            return;
        }
        // The C++ side reads `(w + 7) / 8 * h` bytes, so a short slice would
        // walk off the end of ours.
        let needed = (size.width as usize).div_ceil(8) * size.height as usize;
        if data.len() < needed {
            return;
        }
        unsafe {
            raw::xpui_fui_draw_image(data.as_ptr(), origin.x, origin.y, size.width, size.height)
        }
    }

    fn draw_icon(&self, origin: Point, icon: IconRef) {
        raw::xpui_fui_draw_icon(icon.kind, icon.variant, icon.size, origin.x, origin.y);
    }

    fn icon_size(&self, icon: IconRef) -> i32 {
        raw::xpui_fui_icon_size(icon.kind, icon.variant, icon.size)
    }
}

impl<P: Platform> TextMetrics for Backend<P> {
    fn font(&self, role: FontRole) -> FontId {
        FontId(raw::xpui_fui_font(match role {
            FontRole::Ui => 0,
            FontRole::UiSmall => 1,
            FontRole::Reader => 2,
        }))
    }

    fn text_width(&self, font: FontId, text: &str, style: FontStyle) -> i32 {
        if !font.is_available() {
            return 0;
        }
        let text = as_c(text);
        unsafe { raw::xpui_fui_text_width(font.0, text.as_ptr().cast(), style as u8) }
    }

    fn line_height(&self, font: FontId) -> i32 {
        if !font.is_available() {
            return 0;
        }
        raw::xpui_fui_line_height(font.0)
    }
}

impl<P: Platform> Chrome for Backend<P> {
    fn metric(&self, metric: ThemeMetric) -> i32 {
        raw::xpui_fui_metric(metric as u8)
    }

    fn draw_header(&self, title: Option<&str>, subtitle: Option<&str>) {
        let title = optional(title);
        let subtitle = optional(subtitle);
        unsafe { raw::xpui_fui_draw_header(ptr_of(&title), ptr_of(&subtitle)) }
    }

    fn draw_sub_header(&self, rect: Rect, label: &str, right: Option<&str>) {
        let label = as_c(label);
        let right = optional(right);
        unsafe {
            raw::xpui_fui_draw_sub_header(
                rect.x(),
                rect.y(),
                rect.width(),
                rect.height(),
                label.as_ptr().cast(),
                ptr_of(&right),
            )
        }
    }

    fn draw_button_hints(&self, back: &Hint, confirm: &Hint, previous: &Hint, next: &Hint) {
        // `None` from `label()` means "your own standard label for this slot",
        // which crosses as null. `Some("")` is a slot the screen blanked, and
        // crosses as an empty string — a different thing.
        let slots = [
            optional(back.label()),
            optional(confirm.label()),
            optional(previous.label()),
            optional(next.label()),
        ];
        unsafe {
            raw::xpui_fui_draw_button_hints(
                ptr_of(&slots[0]),
                ptr_of(&slots[1]),
                ptr_of(&slots[2]),
                ptr_of(&slots[3]),
            )
        }
    }

    fn draw_progress_bar(&self, rect: Rect, current: u32, total: u32) {
        raw::xpui_fui_draw_progress_bar(
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            current,
            total,
        );
    }

    fn draw_slider(&self, rect: Rect, value: i32, max: i32) {
        raw::xpui_fui_draw_slider(rect.x(), rect.y(), rect.width(), rect.height(), value, max);
    }

    fn draw_scroll_indicator(&self, rect: Rect, content: i32, visible: i32, offset: i32) {
        raw::xpui_fui_draw_scroll_indicator(
            rect.x(),
            rect.y(),
            rect.width(),
            rect.height(),
            content,
            visible,
            offset,
        );
    }

    fn draw_list<'a>(
        &self,
        rect: Rect,
        rows: usize,
        selected: i32,
        row: &dyn Fn(usize, RowField) -> Option<&'a str>,
    ) {
        // Converted before the call, and kept alive across it: FreeInkUI's
        // props borrow these pointers rather than copying, so nothing may
        // outlive `cells` and `cells` may not move.
        let cells = Cells::rows(rows, row);
        unsafe {
            raw::xpui_fui_draw_list(
                rect.x(),
                rect.y(),
                rect.width(),
                rect.height(),
                rows as i32,
                selected,
                cell_trampoline,
                cells.as_context(),
            )
        }
        drop(cells);
    }

    fn draw_option_popup<'a>(
        &self,
        title: &str,
        options: &dyn Fn(usize) -> Option<&'a str>,
        count: usize,
        selected: i32,
    ) {
        let title = as_c(title);
        let cells = Cells::options(count, options);
        unsafe {
            raw::xpui_fui_draw_option_popup(
                title.as_ptr().cast(),
                count as i32,
                selected,
                cell_trampoline,
                cells.as_context(),
            )
        }
        drop(cells);
    }

    fn option_popup_row_rect<'a>(
        &self,
        title: &str,
        _options: &dyn Fn(usize) -> Option<&'a str>,
        count: usize,
        index: usize,
    ) -> Option<Rect> {
        if index >= count {
            return None;
        }
        let title = as_c(title);
        let mut out = [0i32; 4];
        // Safety: `out` is four `i32`s, which is what the C++ side writes.
        let ok = unsafe {
            raw::xpui_fui_option_popup_row_rect(
                title.as_ptr().cast(),
                count as i32,
                index as i32,
                out.as_mut_ptr(),
            )
        };
        (ok != 0).then(|| Rect::new(out[0], out[1], out[2], out[3]))
    }

    fn request_update(&self) {
        raw::xpui_fui_request_update();
    }
}

impl<P: Platform> InputSource for Backend<P> {
    fn was_pressed(&self, button: Button) -> bool {
        self.platform.was_pressed(button)
    }

    fn is_pressed(&self, button: Button) -> bool {
        self.platform.is_pressed(button)
    }

    fn was_released(&self, button: Button) -> bool {
        self.platform.was_released(button)
    }

    fn has_touch(&self) -> bool {
        self.platform.has_touch()
    }

    fn tap(&self) -> Option<Point> {
        self.platform.tap()
    }

    fn touch_held(&self) -> Option<Point> {
        self.platform.touch_held()
    }

    fn touch_released(&self) -> bool {
        self.platform.touch_released()
    }

    fn swipe(&self) -> SwipeDir {
        self.platform.swipe()
    }

    fn was_back_gesture(&self) -> bool {
        self.platform.was_back_gesture()
    }

    fn was_home_gesture(&self) -> bool {
        self.platform.was_home_gesture()
    }

    fn swipe_moves_selection(&self) -> bool {
        self.platform.swipe_moves_selection()
    }
}

impl<P: Platform> Clock for Backend<P> {
    fn millis(&self) -> u32 {
        self.platform.millis()
    }
}

// A missing trait is caught here rather than at the install site.
const _: fn() = || {
    fn assert_host<T: xpui::host::Host>() {}
    assert_host::<Backend<NoInput>>();
};

/// Keeps the trampoline's signature honest against the ABI.
const _: raw::CellFn = cell_trampoline;

/// Unused, but it keeps `c_void` imported where the ABI needs it.
#[doc(hidden)]
pub type Context = *mut c_void;
