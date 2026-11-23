//! The backend against the recording doubles: what each host trait sends to
//! dessen, and whole apps run through `run` with scripted events.

use dessen::{
    DSN_BITMAP_MASK, DSN_BLACK, DSN_EV_BACK, DSN_EV_CONFIRM, DSN_EV_LONG_PRESS, DSN_EV_NEXT,
    DSN_EV_PREV, DSN_EV_SIDEWAYS, DSN_EV_TAP, DSN_FONT_BODY, DSN_FONT_SMALL, DSN_SCRIM,
    DSN_TEXT_BOLD,
};
use xpui::host::{Canvas, FontRole, FontStyle, InputSource, TextMetrics};
use xpui::{
    App, Button, List, ListRow, Modifiers, NavigationScreen, Point, Rect, Screen, Size, Text, View,
    vstack,
};
use xpui_dessen::testing::{self, Op};
use xpui_dessen::{Backend, Device};

const PORTRAIT: Device = Device {
    width: 480,
    height: 800,
    has_touch: true,
    has_buttons: true,
    hint_height: 0,
};

/// A buttons-only board, whose host draws hints in a 40-pixel band.
const BUTTONS_ONLY: Device = Device {
    has_touch: false,
    hint_height: 40,
    ..PORTRAIT
};

fn hints(ops: &[Op]) -> Option<[String; 4]> {
    ops.iter().find_map(|op| match op {
        Op::Hints(labels) => Some(labels.clone()),
        _ => None,
    })
}

fn install(device: Device) -> &'static Backend {
    testing::reset(device);
    xpui_dessen::install()
}

fn tap(x: u32, y: u32) -> u32 {
    DSN_EV_TAP as u32 | x << 8 | y << 20
}

fn texts(ops: &[Op]) -> Vec<(i32, i32, String)> {
    let text = |op: &Op| match op {
        Op::Text(x, y, s, _, _) => Some((*x, *y, s.clone())),
        _ => None,
    };
    ops.iter().filter_map(text).collect()
}

fn shows(ops: &[Op], label: &str) -> bool {
    texts(ops).iter().any(|(_, _, s)| s == label)
}

#[test]
fn text_is_drawn_top_left_in_the_role_font() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    let ui = backend.font(FontRole::Ui);
    let small = backend.font(FontRole::UiSmall);
    assert!(ui.is_available() && small.is_available(), "no id may be 0");
    assert_ne!(ui, small);

    backend.draw_text(Point::new(12, 34), "Hello", ui, FontStyle::Bold);
    let drawn = Op::Text(12, 34, "Hello".into(), DSN_FONT_BODY, DSN_TEXT_BOLD);
    assert_eq!(testing::take_ops(), vec![drawn]);

    let width = testing::glyph_width(DSN_FONT_SMALL);
    assert_eq!(
        backend.text_width(small, "abc", FontStyle::Regular),
        3 * width
    );
    assert_eq!(
        backend.line_height(small),
        testing::line_height(DSN_FONT_SMALL)
    );
}

#[test]
fn images_are_drawn_as_dessen_masks_without_a_copy() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    // 10 pixels wide: two bytes a row. xpui's bit 0 is ink, and so is the
    // mask format's, so the bytes reach dessen as they are, inked black.
    let image = [0x00, 0xff, 0xf0, 0x0f];
    backend.draw_image(Point::new(5, 6), &image, Size::new(10, 2));
    let drawn = Op::Bitmap(5, 6, 10, 2, image.to_vec(), DSN_BITMAP_MASK, DSN_BLACK);
    assert_eq!(testing::take_ops(), vec![drawn]);

    // A buffer shorter than its size claims is refused whole.
    backend.draw_image(Point::new(0, 0), &[0x00], Size::new(10, 2));
    assert!(testing::take_ops().is_empty());
}

#[derive(Clone, Copy)]
enum Msg {
    Frontlight(bool),
    Sleep,
}

struct Settings {
    frontlight: bool,
    slept: bool,
}

impl Screen for Settings {
    type Message = Msg;

    fn body(&self) -> impl View<Msg> {
        let sleep = if self.slept { "now" } else { "5 min" };
        NavigationScreen::new(
            List::new()
                .push(
                    ListRow::toggle("Frontlight", self.frontlight, "On", "Off")
                        .on_tap(Msg::Frontlight(!self.frontlight)),
                )
                .push(ListRow::new("Sleep after").value(sleep).on_tap(Msg::Sleep)),
        )
    }

    fn update(&mut self, message: Msg) {
        match message {
            Msg::Frontlight(on) => self.frontlight = on,
            Msg::Sleep => self.slept = true,
        }
    }

    fn title(&self) -> Option<&'static str> {
        Some("Settings")
    }
}

fn settings() -> App {
    App::new(Settings {
        frontlight: false,
        slept: false,
    })
}

#[test]
fn a_list_screen_paints_and_answers_buttons() {
    let _guard = testing::lock();
    let backend = install(BUTTONS_ONLY);
    testing::events(&[
        DSN_EV_CONFIRM as u32,
        DSN_EV_NEXT as u32,
        DSN_EV_CONFIRM as u32,
    ]);
    assert_eq!(xpui_dessen::run(backend, settings()), 0);

    let ops = testing::take_ops();
    let frames: Vec<&[Op]> = ops.split(|op| *op == Op::Refresh).collect();
    // The first frame, then one per event that changed something.
    let first = frames[0];
    assert!(shows(first, "Settings") && shows(first, "Frontlight") && shows(first, "Off"));
    // The host draws the hints; the backend only says what each action is.
    let labels = hints(first).expect("a board whose host shows hints gets them");
    assert_eq!([labels[0].as_str(), labels[1].as_str()], ["Back", "Select"]);
    assert!(!shows(first, "Back"), "the backend draws no hints itself");
    assert!(shows(frames[1], "On"), "Confirm toggled the focused row");
    let last = frames[frames.len() - 2];
    assert!(
        shows(last, "now"),
        "Next moved focus, Confirm acted on the second row"
    );
}

#[test]
fn a_tap_acts_on_the_row_under_it() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    xpui_dessen::run(backend, settings());
    let label = texts(&testing::take_ops())
        .into_iter()
        .find(|(_, _, s)| s == "Sleep after");
    let (x, y, _) = label.expect("the row is painted");

    testing::events(&[tap(x as u32 + 2, y as u32 + 2)]);
    xpui_dessen::run(backend, settings());
    assert!(shows(&testing::take_ops(), "now"));
}

#[test]
fn back_on_the_root_screen_ends_the_app() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    testing::events(&[DSN_EV_BACK as u32, DSN_EV_CONFIRM as u32]);
    xpui_dessen::run(backend, settings());
    let ops = testing::take_ops();
    assert!(
        !shows(&ops, "On"),
        "the Confirm after Back never reached the app"
    );
}

struct Holdable {
    presses: u32,
}

impl Screen for Holdable {
    type Message = ();

    fn body(&self) -> impl View<()> {
        let label = if self.presses == 0 {
            String::from("Hold me")
        } else {
            format!("Held {}", self.presses)
        };
        NavigationScreen::new(vstack![0; Text::new(label).on_long_press(())])
    }

    fn update(&mut self, _message: ()) {
        self.presses += 1;
    }
}

#[test]
fn a_long_press_is_four_frames_ending_in_a_lift() {
    let point = Point::new(30, 40);
    let event = DSN_EV_LONG_PRESS as u32 | 30 << 8 | 40 << 20;
    let (frames, len) = xpui_dessen::input::frames(event);
    assert_eq!(len, 4);
    assert!(
        frames[..3]
            .iter()
            .all(|f| f.held == Some(point) && f.tap.is_none())
    );
    assert_eq!(
        (frames[3].tap, frames[3].released, frames[3].held),
        (Some(point), true, None)
    );
    // Two steps reach xpui's 500 ms threshold, each under its 500 ms blind guard.
    const {
        use xpui_dessen::input::{EVENT_GAP_MS, HOLD_STEP_MS};
        assert!(2 * HOLD_STEP_MS == 500 && HOLD_STEP_MS < 500 && EVENT_GAP_MS >= 500);
    }
    assert_eq!(
        xpui_dessen::input::frames(0xff).1,
        0,
        "an unknown event is no frames"
    );
}

#[test]
fn a_long_press_acts_once() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    xpui_dessen::run(backend, App::new(Holdable { presses: 0 }));
    let label = texts(&testing::take_ops())
        .into_iter()
        .find(|(_, _, s)| s == "Hold me");
    let (x, y, _) = label.expect("the label is painted");

    let at = (x as u32 + 2) << 8 | (y as u32 + 2) << 20;
    testing::events(&[DSN_EV_LONG_PRESS as u32 | at]);
    xpui_dessen::run(backend, App::new(Holdable { presses: 0 }));
    let ops = testing::take_ops();
    assert!(
        shows(&ops, "Held 1") && !shows(&ops, "Held 2"),
        "the hold, and not its lift too"
    );
}

#[test]
fn a_long_press_where_nothing_holds_is_a_tap() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    xpui_dessen::run(backend, settings());
    let label = texts(&testing::take_ops())
        .into_iter()
        .find(|(_, _, s)| s == "Sleep after");
    let (x, y, _) = label.expect("the row is painted");

    testing::events(&[DSN_EV_LONG_PRESS as u32 | (x as u32 + 2) << 8 | (y as u32 + 2) << 20]);
    xpui_dessen::run(backend, settings());
    assert!(shows(&testing::take_ops(), "now"));
}

#[test]
fn where_the_host_shows_no_hints_none_are_asked_for() {
    let _guard = testing::lock();
    // hint_height 0: a touch board, keys or not, or a board without keys.
    for (has_touch, has_buttons) in [(true, true), (true, false), (false, false)] {
        let device = Device {
            has_touch,
            has_buttons,
            ..PORTRAIT
        };
        let backend = install(device);
        assert_eq!(backend.device(), device);
        xpui_dessen::run(backend, settings());
        let ops = testing::take_ops();
        assert!(shows(&ops, "Frontlight") && hints(&ops).is_none());
    }
}

#[test]
fn the_hint_band_comes_from_dsn_device() {
    let _guard = testing::lock();
    let backend = install(BUTTONS_ONLY);
    assert_eq!(backend.device().hint_height, 40);
    assert_eq!(
        xpui::host::Chrome::metric(backend, xpui::host::ThemeMetric::ButtonHintsHeight),
        40
    );
}

#[test]
fn a_clip_reaches_dessen_and_lifting_it_is_a_negative_width() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    testing::take_ops();
    backend.set_clip(Some(Rect::new(4, 8, 100, 50)));
    backend.set_clip(Some(Rect::new(4, 8, -3, 50)));
    backend.set_clip(None);
    backend.scrim(Rect::new(0, 0, 10, 10));
    assert_eq!(
        testing::take_ops(),
        [
            Op::Clip(4, 8, 100, 50),
            Op::Clip(4, 8, 0, 50),
            Op::Clip(0, 0, -1, -1),
            Op::Fill(0, 0, 10, 10, DSN_SCRIM),
        ]
    );
}

#[test]
fn a_sideways_key_is_left_or_right_and_turns_the_pair_on() {
    let _guard = testing::lock();
    let backend = install(PORTRAIT);
    let button = |event: u32| xpui_dessen::input::frames(event).0[0].button;
    assert_eq!(button(DSN_EV_PREV as u32), Some(Button::Up));
    assert_eq!(
        button(DSN_EV_NEXT as u32 | DSN_EV_SIDEWAYS),
        Some(Button::Right)
    );
    assert_eq!(
        button(DSN_EV_PREV as u32 | DSN_EV_SIDEWAYS),
        Some(Button::Left)
    );

    assert!(!backend.has_left_right_keys(), "unknown until one arrives");
    testing::events(&[DSN_EV_NEXT as u32 | DSN_EV_SIDEWAYS]);
    xpui_dessen::run(backend, settings());
    assert!(backend.has_left_right_keys());
}
