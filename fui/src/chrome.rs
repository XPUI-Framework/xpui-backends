//! The components FreeInkUI already knows how to draw.

use xpui::Rect;
use xpui::host::{Chrome, Hint, RowField, ThemeMetric};

use crate::backend::Backend;
use crate::cells::{Cells, cell_trampoline};
use crate::marshal::{as_c, optional, ptr_of};
use crate::platform::Platform;
use crate::raw;

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
