//! An [`xpui`] backend for **dessen** apps.
//!
//! Dessen runs small sandboxed RV32IM apps inside CrossPoint and FreeInk
//! reader firmware, launched from the SD card. An app reaches the device only
//! through dessen's calls, which the firmware forwards to FreeInkUI's
//! `DrawTarget`; this crate answers `xpui`'s host traits with those calls, and
//! takes `Chrome` from [`xpui_chrome`]. One `app.bin` then runs on every
//! board, whatever its chip, because the chip only ever runs the VM.
//!
//! An app names a function building its [`App`] with [`main!`], which
//! installs the backend and hands the app to [`run`], which owns the loop:
//! wait for an event, feed it as frames, repaint if anything changed. See
//! `README.md` for a whole app.
//!
//! Built with `--cfg xpui_static_host`, as every dessen app is, the backend is
//! linked rather than installed: it lives in a `static`, and `xpui` reaches
//! each method by name, so an app carries only the drawing it uses.

#![cfg_attr(target_os = "none", no_std)]
#![deny(missing_docs)]

extern crate alloc;

#[cfg(not(xpui_static_host))]
use alloc::boxed::Box;
use core::cell::Cell;

use xpui::App;
use xpui_chrome::{Labels, Metrics};

mod calls;
mod canvas;
mod chrome;
pub mod input;
#[cfg(not(target_arch = "riscv32"))]
pub mod testing;

use calls::{dsn_device, dsn_refresh, dsn_screen, dsn_wait_event};
use input::{EVENT_GAP_MS, Frame, HOLD_STEP_MS};

/// What the backend needs to know about the device, read from dessen.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// The drawable area's width: the safe area, inside any bezel.
    pub width: i32,
    /// The drawable area's height.
    pub height: i32,
    /// Whether taps and swipes arrive.
    pub has_touch: bool,
    /// Whether buttons arrive.
    pub has_buttons: bool,
    /// The height of the band along the bottom where the host draws button
    /// hints, from `dsn_device`; 0 where it shows none (a touch board, a board
    /// without keys, or keys not along the bottom as the screen is turned).
    /// The backend keeps that band clear and hands the host each action's
    /// label with `dsn_button_hints`; the host draws them in its own look, over
    /// its own keys. Where it is 0 no band is reserved and no hints are asked
    /// for.
    pub hint_height: i32,
}

impl Device {
    /// The running device, from `dsn_screen` and `dsn_device`.
    pub fn read() -> Device {
        let packed = dsn_screen();
        // dsn_device_info: has_touch at byte 18, has_buttons at 19,
        // hint_height at 22 (see sdk/dessen_abi.h).
        let mut info = [0u8; 24];
        let filled = dsn_device(&mut info) as usize;
        Device {
            width: (packed >> 16) as i32,
            height: (packed & 0xffff) as i32,
            has_touch: filled >= 20 && info[18] != 0,
            has_buttons: filled < 20 || info[19] != 0,
            hint_height: if filled >= 24 {
                i32::from(u16::from_le_bytes([info[22], info[23]]))
            } else {
                0
            },
        }
    }
}

/// An `xpui` host over dessen's calls.
pub struct Backend {
    device: Device,
    metrics: Metrics,
    labels: Labels,
    frame: Cell<Frame>,
    clock: Cell<u32>,
    sideways: Cell<bool>,
}

// Safety: a dessen app is one thread with no interrupts, and `xpui` reads the
// host only from inside a frame, so no two accesses to the cells can overlap.
// Only an installed host must be `Sync`; a linked one is reached by name.
#[cfg(not(xpui_static_host))]
unsafe impl Sync for Backend {}

impl Backend {
    /// A backend for `device`, with chrome sized to its panel.
    pub fn new(device: Device) -> Backend {
        // The host draws the hints, in its own look; the backend only keeps
        // its band clear. Where it shows none, the content gains the space.
        let mut metrics = Metrics::for_panel(device.width, device.height).without_button_hints();
        metrics.button_hints_height = device.hint_height.clamp(0, device.height);
        Backend {
            device,
            metrics,
            labels: Labels::for_panel(device.width, device.height),
            frame: Cell::new(Frame::default()),
            clock: Cell::new(0),
            sideways: Cell::new(false),
        }
    }

    /// The device this backend was built for.
    pub fn device(&self) -> Device {
        self.device
    }
}

/// Builds a backend for the running device and installs it as `xpui`'s host.
///
/// Call it once, before the first screen is built; [`main!`] does. The
/// backend lives for the rest of the app: in a `static` when it is linked
/// (`--cfg xpui_static_host`), leaked onto the heap when it is installed.
pub fn install() -> &'static Backend {
    #[cfg(xpui_static_host)]
    {
        // A second call keeps the first backend: the device does not change.
        let _ = linked::BACKEND.0.set(Backend::new(Device::read()));
        linked::backend()
    }
    #[cfg(not(xpui_static_host))]
    {
        let backend: &'static Backend = Box::leak(Box::new(Backend::new(Device::read())));
        // Safety: once, at startup, before any frame; a dessen app has one thread.
        unsafe { xpui::host::install(backend) };
        backend
    }
}

/// The linked backend: one `static`, set by [`install`] before the first frame.
#[cfg(xpui_static_host)]
mod linked {
    use core::cell::OnceCell;

    use super::Backend;

    /// Where [`install`](super::install) puts the app's one backend.
    pub(crate) struct Slot(pub(crate) OnceCell<Backend>);

    // Safety: a dessen app is one thread with no interrupts, and the slot is
    // written once, by `install`, before anything reads it.
    unsafe impl Sync for Slot {}

    pub(crate) static BACKEND: Slot = Slot(OnceCell::new());

    /// The backend, for every host call `xpui` makes.
    #[inline(always)]
    pub(crate) fn backend() -> &'static Backend {
        match BACKEND.0.get() {
            Some(backend) => backend,
            None => panic!("xpui_dessen::install was never called"),
        }
    }

    xpui::link_host!(backend());
}

/// Writes the app's entry point: installs the backend, builds the app with
/// `$app`, a `fn() -> xpui::App`, and runs it until it finishes.
///
/// ```text
/// fn app() -> xpui::App {
///     xpui::App::new(Hello)
/// }
/// xpui_dessen::main!(app);
/// ```
///
/// `$app` runs after the backend is installed, so it may measure text or read
/// the theme while it builds the first screen.
#[macro_export]
macro_rules! main {
    ($app:path) => {
        fn __xpui_dessen_main() -> i32 {
            let backend = $crate::install();
            $crate::run(backend, $app())
        }
        $crate::__dessen::main!(__xpui_dessen_main);
    };
}

#[doc(hidden)]
pub use dessen as __dessen;

/// Runs `app` until its last screen finishes, then returns 0 for the app to
/// exit with.
///
/// Paints the first frame, then for each dessen event feeds its frames and
/// refreshes the panel when anything repainted. Back on the root screen
/// finishes it, which is how a dessen app is expected to quit. An event of 0
/// also ends the loop: dessen never sends one, the host doubles do when their
/// script runs out.
pub fn run(backend: &Backend, mut app: App) -> i32 {
    app.render();
    dsn_refresh();
    while app.is_running() {
        let event = dsn_wait_event();
        if event == 0 {
            break;
        }
        let (frames, len) = input::frames(event);
        for (index, frame) in frames[..len].iter().enumerate() {
            backend.set_frame(
                *frame,
                if index == 0 {
                    EVENT_GAP_MS
                } else {
                    HOLD_STEP_MS
                },
            );
            app.tick();
        }
        backend.set_frame(Frame::default(), 0);
        if app.render_if_dirty() {
            dsn_refresh();
        }
    }
    0
}

#[cfg(doctest)]
mod guides {
    #[doc = include_str!("../README.md")]
    pub mod readme {}
    #[doc = include_str!("../docs/coverage.md")]
    pub mod coverage {}
    #[doc = include_str!("../docs/reference.md")]
    pub mod reference {}
    #[doc = include_str!("../docs/reference/backend.md")]
    pub mod reference_backend {}
    #[doc = include_str!("../docs/reference/input.md")]
    pub mod reference_input {}
    #[doc = include_str!("../docs/reference/testing.md")]
    pub mod reference_testing {}
}
