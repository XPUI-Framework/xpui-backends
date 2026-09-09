//! Strings for the components the C++ side paints itself.
//!
//! The C++ side asks for each cell through a callback, and a callback handed a
//! borrowed `&str` cannot produce a NUL-terminated pointer. So every cell is
//! converted once before the call and the trampoline just indexes this — which
//! also means one conversion per string per frame rather than one per
//! callback, and FreeInkUI asks for each row more than once.

use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::c_void;

use xpui::host::RowField;

/// Every field of every row, flattened, in the order the theme asks for them.
pub(crate) struct Cells {
    cells: Vec<Option<CString>>,
    stride: usize,
}

/// The three fields, in the order `field` numbers them across the ABI.
const FIELDS: [RowField; 3] = [RowField::Title, RowField::Subtitle, RowField::Value];

fn to_c(text: &str) -> CString {
    // An interior NUL cannot come from our own widgets. Blanking beats
    // refusing to draw, and beats truncating silently at the NUL.
    CString::new(text).unwrap_or_default()
}

impl Cells {
    pub(crate) fn rows<'a>(rows: usize, row: &dyn Fn(usize, RowField) -> Option<&'a str>) -> Self {
        let mut cells = Vec::with_capacity(rows * FIELDS.len());
        for index in 0..rows {
            for field in FIELDS {
                cells.push(row(index, field).map(to_c));
            }
        }
        Cells {
            cells,
            stride: FIELDS.len(),
        }
    }

    /// A one-field-per-row table, for a dialog's options.
    pub(crate) fn options<'a>(count: usize, option: &dyn Fn(usize) -> Option<&'a str>) -> Self {
        Cells {
            cells: (0..count).map(|index| option(index).map(to_c)).collect(),
            stride: 1,
        }
    }

    fn get(&self, index: i32, field: i32) -> *const u8 {
        let (Ok(index), Ok(field)) = (usize::try_from(index), usize::try_from(field)) else {
            return core::ptr::null();
        };
        if field >= self.stride {
            return core::ptr::null();
        }
        match self.cells.get(index * self.stride + field) {
            // Null means "this row has no such field", which is how the theme
            // decides between a one- and a two-line row. An empty string is a
            // different thing and would make every row tall.
            Some(Some(text)) => text.as_ptr().cast(),
            _ => core::ptr::null(),
        }
    }

    /// A context pointer and a trampoline, for one call across the boundary.
    pub(crate) fn as_context(&self) -> *mut c_void {
        (self as *const Cells as *mut Cells).cast()
    }
}

/// Safe to call, and only meaningful when `ctx` is the pointer from
/// [`Cells::as_context`] of a `Cells` still alive for the call: anything
/// else is answered with null rather than dereferenced.
pub(crate) extern "C" fn cell_trampoline(ctx: *mut c_void, index: i32, field: i32) -> *const u8 {
    if ctx.is_null() {
        return core::ptr::null();
    }
    // Safety: the only caller is the C++ side, inside a call whose argument
    // came from `as_context` on a `Cells` that outlives it.
    let cells = unsafe { &*(ctx as *const Cells) };
    cells.get(index, field)
}
