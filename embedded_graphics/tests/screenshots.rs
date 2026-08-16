//! End-to-end: a screen, through the framework, through this backend, onto
//! pixels.
//!
//! Everything else tests one layer. This tests all of them at once, which is
//! the only way to catch the failures that live between layers — a component
//! measured with one font and painted with another, chrome drawn under content
//! instead of over it, a clip that never gets lifted.
//!
//! Each case compares the whole panel against a committed PNG in
//! `tests/screenshots/`, pixel for pixel. The numeric assertions beside them
//! pin the things a picture cannot argue about on its own.
//!
//! ```bash
//! UPDATE_SNAPSHOTS=1 cargo test -p xpui-embedded-graphics
//! open crates/backend/embedded_graphics/tests/screenshots/
//! ```

use std::sync::{Mutex, MutexGuard};

use xpui::screen::Screen;
use xpui::{
    App, Button, Hint, List, ListRow, Modal, NavigationScreen, ProgressBar, Scrim, ScrollView,
    Section, Slider, Stepper, Text, Toggle, View, hstack, vstack,
};
use xpui_eg::Framebuffer as TestDisplay;
use xpui_eg::{Backend, Palette, assert_screenshot};

/// A portrait e-reader panel.
const WIDTH: i32 = 480;
const HEIGHT: i32 = 800;

/// The installed host is process-wide, so only one of these runs at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Installs a fresh backend over a fresh panel.
fn install() -> &'static Backend<TestDisplay> {
    let backend = Backend::leak(TestDisplay::new(WIDTH, HEIGHT), Palette::INK_IS_ON);
    // Safety: serialised by `SERIAL`, and nothing has rendered on this backend.
    unsafe { xpui::host::install(backend) };
    backend
}

/// Renders `screen` and compares the panel against `name`'s golden.
fn shoot<S: Screen + 'static>(name: &str, screen: S) -> &'static Backend<TestDisplay> {
    let backend = install();
    let mut app = App::new(screen);
    app.render();
    capture(backend, name);
    backend
}

fn capture(backend: &'static Backend<TestDisplay>, name: &str) {
    backend.with_display(|display| assert_screenshot(name, display));
}

// -- screens ---------------------------------------------------------------

struct Settings {
    hyphenation: bool,
}

impl Screen for Settings {
    type Message = ();

    fn body(&self) -> impl View<Self::Message> {
        NavigationScreen::new(vstack![12;
            List::new()
                .push(ListRow::new("Wi-Fi").value("Off").on_tap(()))
                .push(ListRow::new("Storage").subtitle("3.1 GB free").on_tap(()))
                .push(ListRow::new("Language").value("English").on_tap(())),
            Toggle::new("Hyphenation", self.hyphenation, "On", "Off").on_change(|_| ()),
            ProgressBar::percent(62),
        ])
        .title("Settings")
    }

    fn update(&mut self, _message: Self::Message) {}

    fn title(&self) -> Option<&'static str> {
        Some("Settings")
    }
}

struct Light {
    level: i32,
}

impl Screen for Light {
    type Message = i32;

    fn body(&self) -> impl View<Self::Message> {
        NavigationScreen::new(vstack![16;
            hstack![8; Text::new("Brightness").bold(), Text::new("60%")],
            Stepper::new(self.level).on_change(|v| v),
            Slider::new(self.level, 100).on_change(|v| v),
        ])
        .title("Light")
        .hints(Hint::Standard, Hint::text("Save"), Hint::None, Hint::None)
    }

    fn update(&mut self, message: Self::Message) {
        self.level = message.clamp(0, 100);
    }

    fn title(&self) -> Option<&'static str> {
        Some("Light")
    }
}

struct Picker {
    open: bool,
}

impl Screen for Picker {
    type Message = usize;

    fn body(&self) -> impl View<Self::Message> {
        NavigationScreen::new(
            List::new()
                .push(ListRow::new("Font").value("Serif").on_tap(0))
                .push(ListRow::new("Size").value("16").on_tap(1)),
        )
        .title("Reading")
        .overlay_if(
            self.open,
            Modal::picker("Font", ["Serif", "Sans", "Mono"])
                .selected(1)
                .on_select(|i| i)
                .scrim(Scrim::Dim),
        )
    }

    fn update(&mut self, _message: Self::Message) {}

    fn title(&self) -> Option<&'static str> {
        Some("Reading")
    }
}

struct LongPage;

impl Screen for LongPage {
    type Message = usize;

    fn body(&self) -> impl View<Self::Message> {
        NavigationScreen::new(ScrollView::new(vstack![10;
            Section::new("Display", List::new()
                .push(ListRow::new("Brightness").value("60%").on_tap(0))
                .push(ListRow::new("Warmth").value("30%").on_tap(1))
                .push(ListRow::new("Invert").value("Off").on_tap(2))),
            Section::new("Reading", List::new()
                .push(ListRow::new("Font").value("Serif").on_tap(3))
                .push(ListRow::new("Margins").value("Medium").on_tap(4))
                .push(ListRow::new("Spacing").value("1.4").on_tap(5))),
            Section::new("System", List::new()
                .push(ListRow::new("Language").value("English").on_tap(6))
                .push(ListRow::new("Sleep").value("15 min").on_tap(7))
                .push(ListRow::new("Storage").value("3.1 GB").on_tap(8))),
            Section::new("About", List::new()
                .push(ListRow::new("Firmware").value("1.4.2").on_tap(9))
                .push(ListRow::new("Serial").value("X4-0001").on_tap(10))
                .push(ListRow::new("Licences").on_tap(11))),
            // Past the bottom of the panel on purpose. A page that fits never
            // scrolls, and every assertion below would pass without the thing
            // they are named after ever happening.
            Section::new("Network", List::new()
                .push(ListRow::new("Wi-Fi").value("Off").on_tap(12))
                .push(ListRow::new("Sync").value("Never").on_tap(13))
                .push(ListRow::new("Proxy").value("None").on_tap(14))),
        ]))
        .title("All settings")
    }

    fn update(&mut self, _message: Self::Message) {}

    fn title(&self) -> Option<&'static str> {
        Some("All settings")
    }
}

// -- the shots -------------------------------------------------------------

#[test]
fn settings_screen() {
    let _guard = serial();
    let backend = shoot("settings", Settings { hyphenation: true });

    // The golden proves the frame is the one that was blessed; these say what
    // makes it the right frame, so a reviewer blessing a change can tell.
    // "Some ink somewhere" would be true of any frame at all, so each one
    // names a band and what belongs in it.
    let header = backend.with_display(|d| d.ink_in(0, 0, WIDTH, 56));
    let hints = backend.with_display(|d| d.ink_in(0, HEIGHT - 40, WIDTH, 40));
    let between = backend.with_display(|d| d.ink_in(0, 56, WIDTH, HEIGHT - 96));

    assert!(header > 0, "the header band has a title in it");
    assert!(hints > 0, "the hint band has its four labels");
    assert!(between > header, "and the content is the bulk of the frame");

    // The header's rule spans the panel, so the band's ink is not just a word.
    assert!(
        header > WIDTH as usize,
        "the header drew its rule as well as its title ({header} pixels)"
    );
}

#[test]
fn brightness_screen() {
    let _guard = serial();
    shoot("brightness", Light { level: 60 });
}

#[test]
fn reading_screen_without_a_dialog() {
    let _guard = serial();
    shoot("picker_closed", Picker { open: false });
}

/// The dialog dims what is behind it rather than hiding it, so the list must
/// still be partly visible under the scrim.
#[test]
fn reading_screen_with_a_dialog() {
    let _guard = serial();
    let backend = shoot("picker_open", Picker { open: true });

    // Counting ink over the whole panel proves almost nothing: the scrim alone
    // inks one parity, so "more than a quarter" is satisfied by a dialog that
    // never cleared its background at all. The assertion that bites compares
    // *inside* the dialog against the scrim around it.
    let scrim = backend.with_display(|d| d.ink_in(0, 60, WIDTH, 200));
    let dialog = backend.with_display(|d| d.ink_in(120, 330, 240, 120));

    let scrim_density = scrim as f32 / (WIDTH * 200) as f32;
    let dialog_density = dialog as f32 / (240 * 120) as f32;

    assert!(
        scrim_density > 0.4,
        "the scrim darkened what is behind the dialog ({scrim_density:.2})"
    );
    assert!(
        dialog_density < scrim_density / 2.0,
        "and the dialog cleared its own background out of it \
         (dialog {dialog_density:.2} vs scrim {scrim_density:.2})"
    );
}

#[test]
fn a_long_page_scrolled_to_the_top() {
    let _guard = serial();
    let backend = shoot("scrolling_top", LongPage);

    // The indicator is drawn only when the content overflows, so its presence
    // is the proof that this page is long enough to be worth the name.
    let right_edge = backend.with_display(|d| d.ink_in(WIDTH - 8, 60, 8, 700));
    assert!(
        right_edge > 0,
        "a scroll indicator must be visible, or this page fits and nothing here scrolls"
    );
}

/// Walking focus to the bottom scrolls the page. The screenshot is the proof
/// that content moved and the chrome did not.
#[test]
fn a_long_page_scrolled_to_the_bottom() {
    let _guard = serial();
    let backend = install();
    let mut app = App::new(LongPage);
    app.render();

    // Wrap backwards to the last control.
    backend.begin_frame(0);
    backend.press(Button::Up);
    app.tick();
    app.render();

    capture(backend, "scrolling_bottom");

    // The hint band is drawn after the clip is lifted. If the lift were
    // missed, the scrolling content's clip would swallow it.
    assert!(
        backend.with_display(|d| d.ink_in(0, HEIGHT - 40, WIDTH, 40)) > 0,
        "the button hints survived the scroll view's clip"
    );

    // Nothing may paint above the content band: the clip is what keeps the
    // scrolled-up content off the header, and a scroll view that leaked would
    // show its overflow right here.
    let above = backend.with_display(|d| d.ink_in(0, 50, WIDTH, 9));
    assert_eq!(
        above, 0,
        "content scrolled up under the header must be clipped away, not drawn over it"
    );
}
