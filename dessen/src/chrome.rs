//! `Chrome`: xpui-chrome's painting, but for the button hints.
//!
//! Everything is xpui-chrome's own, as `plain_chrome!` would write it, except
//! the hints: those are the host's, drawn by `dsn_button_hints` in its own look
//! and over its own keys, wherever the device has them. The backend says what
//! each action means on this screen, and nothing about where.

use xpui::Rect;
use xpui::host::{Chrome, ControlState, Hint, HintWord, RowField, ThemeMetric};

use crate::Backend;
use crate::calls::dsn_button_hints;

impl Chrome for Backend {
    fn metric(&self, metric: ThemeMetric) -> i32 {
        self.metrics.metric(metric)
    }

    fn draw_header(&self, title: Option<&str>, subtitle: Option<&str>) {
        xpui_chrome::draw_header(&self.metrics, title, subtitle)
    }

    fn draw_sub_header(&self, rect: Rect, label: &str, right: Option<&str>) {
        xpui_chrome::draw_sub_header(&self.metrics, rect, label, right)
    }

    fn draw_button_hints(&self, back: &Hint, confirm: &Hint, previous: &Hint, next: &Hint) {
        if self.device.hint_height <= 0 {
            return;
        }
        // Back, confirm, previous, next: one line each, empty for no hint.
        let mut labels = [0u8; 4 * 33];
        let mut used = 0;
        for (index, hint) in [back, confirm, previous, next].into_iter().enumerate() {
            let label = match hint.label() {
                Some(text) => text,
                None => match hint.word() {
                    HintWord::Standard => self.labels.standard_hints[index],
                    HintWord::Edit => self.labels.mode_hints[0],
                    HintWord::Done => self.labels.mode_hints[1],
                    HintWord::Cancel => self.labels.mode_hints[2],
                },
            };
            if index > 0 {
                labels[used] = b'\n';
                used += 1;
            }
            // Cut to the host's limit on a character boundary.
            let mut end = label.len().min(32);
            while !label.is_char_boundary(end) {
                end -= 1;
            }
            labels[used..used + end].copy_from_slice(&label.as_bytes()[..end]);
            used += end;
        }
        dsn_button_hints(&labels[..used]);
    }

    fn draw_progress_bar(&self, rect: Rect, current: u32, total: u32) {
        xpui_chrome::draw_progress_bar(&self.metrics, rect, current, total)
    }

    fn draw_slider(&self, rect: Rect, value: i32, max: i32, state: ControlState) {
        xpui_chrome::draw_slider(&self.metrics, rect, value, max, state)
    }

    fn draw_scroll_indicator(&self, rect: Rect, content: i32, visible: i32, offset: i32) {
        xpui_chrome::draw_scroll_indicator(&self.metrics, rect, content, visible, offset)
    }

    fn draw_list<'a>(
        &self,
        rect: Rect,
        rows: usize,
        selected: i32,
        row: &dyn Fn(usize, RowField) -> Option<&'a str>,
    ) {
        xpui_chrome::draw_list(&self.metrics, rect, rows, selected, row)
    }

    fn draw_option_popup<'a>(
        &self,
        title: &str,
        options: &dyn Fn(usize) -> Option<&'a str>,
        count: usize,
        selected: i32,
    ) {
        xpui_chrome::draw_option_popup(&self.metrics, title, options, count, selected)
    }

    fn option_popup_row_rect<'a>(
        &self,
        title: &str,
        options: &dyn Fn(usize) -> Option<&'a str>,
        count: usize,
        index: usize,
    ) -> Option<Rect> {
        xpui_chrome::option_popup_row_rect(&self.metrics, title, options, count, index)
    }

    // `run` repaints whenever the framework marks the screen dirty, so there
    // is nothing more to ask for here.
    fn request_update(&self) {}
}
