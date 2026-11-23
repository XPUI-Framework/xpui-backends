//! The dessen calls this backend makes, from one place.
//!
//! On the VM they are `dessen`'s own: one `ecall` each, sanitized by the
//! firmware on the far side. Everywhere else there is no VM to call, so the
//! same names come from [`crate::testing`], which records each call and answers
//! measurements from fixed metrics. Nothing above this module knows which.

#[cfg(target_arch = "riscv32")]
pub(crate) use dessen::{
    dsn_bitmap, dsn_button_hints, dsn_clear, dsn_device, dsn_fill, dsn_line, dsn_line_height,
    dsn_measure_text, dsn_refresh, dsn_screen, dsn_set_clip, dsn_stroke, dsn_text_box,
    dsn_wait_event,
};

#[cfg(not(target_arch = "riscv32"))]
pub(crate) use crate::testing::calls::{
    dsn_bitmap, dsn_button_hints, dsn_clear, dsn_device, dsn_fill, dsn_line, dsn_line_height,
    dsn_measure_text, dsn_refresh, dsn_screen, dsn_set_clip, dsn_stroke, dsn_text_box,
    dsn_wait_event,
};
