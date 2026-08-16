//! What actually crosses the C boundary.
//!
//! This is the half of the FreeInkUI backend that Rust owns. The C++ half is
//! checked by compiling it; everything here is about whether the right bytes
//! arrive — and the distinctions that matter most are the ones a type system
//! cannot see, because on the C side they are all just pointers:
//!
//! - a null label means "host, use your own", an empty one means "leave it
//!   blank", and confusing them puts the wrong word under a button
//! - a null cell means the row has no such field, which is how a one-line row
//!   is told apart from a two-line one
//! - a font this build compiled out must draw nothing rather than substituting

use xpui::host::{
    Canvas, Chrome, FontId, FontRole, FontStyle, Hint, IconRef, RowField, TextMetrics,
};
use xpui::{Point, Rect, Size};
use xpui_fui::testing::{self, Call};
use xpui_fui::{Backend, NoInput};

static PLATFORM: NoInput = NoInput;

fn backend() -> Backend<NoInput> {
    testing::reset();
    Backend::new(&PLATFORM)
}

fn rows(cells: &[(&'static str, Option<&'static str>, Option<&'static str>)]) -> Vec<Cells> {
    cells.to_vec()
}

type Cells = (&'static str, Option<&'static str>, Option<&'static str>);

fn row_fn(cells: Vec<Cells>) -> impl Fn(usize, RowField) -> Option<&'static str> {
    move |index, field| {
        let (title, subtitle, value) = *cells.get(index)?;
        match field {
            RowField::Title => Some(title),
            RowField::Subtitle => subtitle,
            RowField::Value => value,
        }
    }
}

// -- the distinction the whole ABI turns on --------------------------------

/// `Hint::Standard` means "your own label for this slot" and crosses as null.
/// `Hint::None` means "leave it blank" and crosses as an empty string. A shim
/// that treated null as blank would leave every standard slot empty; one that
/// treated blank as null would label a slot the screen asked to hide.
#[test]
fn a_standard_hint_and_a_blank_one_cross_differently() {
    let backend = backend();
    backend.draw_button_hints(
        &Hint::Standard,
        &Hint::text("Save"),
        &Hint::None,
        &Hint::Standard,
    );

    let Some(Call::Hints(slots)) = testing::calls().into_iter().next() else {
        panic!("no hints were drawn: {:?}", testing::calls());
    };

    assert_eq!(slots[0], None, "Standard crosses as null");
    assert_eq!(slots[1].as_deref(), Some("Save"), "Text crosses as itself");
    assert_eq!(
        slots[2].as_deref(),
        Some(""),
        "None crosses as an empty string, which is not null"
    );
    assert_eq!(slots[3], None);
}

/// The same distinction for a header: no title at all is null, not "".
#[test]
fn a_header_with_no_subtitle_sends_null_for_it() {
    let backend = backend();
    backend.draw_header(Some("Settings"), None);

    assert_eq!(
        testing::calls(),
        vec![Call::Header {
            title: Some(String::from("Settings")),
            subtitle: None,
        }]
    );
}

/// And for a row: a missing subtitle is null. An empty one would make every
/// row in the list tall, because the theme decides row height by whether ANY
/// row has one.
#[test]
fn a_row_without_a_subtitle_sends_null_not_empty() {
    let backend = backend();
    let cells = row_fn(rows(&[
        ("Wi-Fi", None, Some("Off")),
        ("Storage", Some("3.1 GB free"), None),
    ]));

    backend.draw_list(Rect::new(0, 0, 400, 400), 2, 0, &cells);

    let Some(Call::List { cells, .. }) = testing::calls().into_iter().next() else {
        panic!("no list was drawn");
    };

    assert_eq!(cells[0][0].as_deref(), Some("Wi-Fi"));
    assert_eq!(cells[0][1], None, "no subtitle crosses as null");
    assert_eq!(cells[0][2].as_deref(), Some("Off"));

    assert_eq!(cells[1][1].as_deref(), Some("3.1 GB free"));
    assert_eq!(cells[1][2], None, "no value crosses as null");
}

/// A row that genuinely has an empty subtitle must still send one, or the
/// screen and the theme disagree about how tall the rows are.
#[test]
fn an_empty_subtitle_is_not_turned_into_null() {
    let backend = backend();
    let cells = row_fn(rows(&[("Only", Some(""), None)]));
    backend.draw_list(Rect::new(0, 0, 400, 400), 1, -1, &cells);

    let Some(Call::List { cells, .. }) = testing::calls().into_iter().next() else {
        panic!("no list was drawn");
    };
    assert_eq!(cells[0][1].as_deref(), Some(""));
}

// -- the callback ----------------------------------------------------------

/// The C++ side pulls every cell back through a callback while the call is
/// running. If the strings it borrows were freed first it would read rubbish,
/// and the test double pulls them exactly as FreeInkUI does.
#[test]
fn every_cell_survives_being_pulled_back_across_the_boundary() {
    let backend = backend();
    let cells = row_fn(rows(&[
        ("One", Some("first"), Some("1")),
        ("Two", Some("second"), Some("2")),
        ("Three", Some("third"), Some("3")),
    ]));

    backend.draw_list(Rect::new(0, 0, 400, 400), 3, 1, &cells);

    let Some(Call::List {
        rows,
        selected,
        cells,
    }) = testing::calls().into_iter().next()
    else {
        panic!("no list was drawn");
    };

    assert_eq!(rows, 3);
    assert_eq!(selected, 1);
    assert_eq!(
        cells,
        vec![
            [
                Some(String::from("One")),
                Some(String::from("first")),
                Some(String::from("1"))
            ],
            [
                Some(String::from("Two")),
                Some(String::from("second")),
                Some(String::from("2"))
            ],
            [
                Some(String::from("Three")),
                Some(String::from("third")),
                Some(String::from("3"))
            ],
        ]
    );
}

#[test]
fn a_dialogs_options_cross_intact() {
    let backend = backend();
    let options = ["Serif", "Sans", "Mono"];
    backend.draw_option_popup("Font", &|i| options.get(i).copied(), options.len(), 2);

    assert_eq!(
        testing::calls(),
        vec![Call::Popup {
            title: String::from("Font"),
            selected: 2,
            options: options.iter().map(|o| Some(String::from(*o))).collect(),
        }]
    );
}

/// Asking past the end must not invent a row, and must not cross the boundary
/// at all — the C++ side would have to guess what to write into `out_xywh`.
#[test]
fn asking_for_a_row_past_the_end_never_reaches_the_shim() {
    let backend = backend();
    let options = ["A", "B"];
    let get = |i: usize| options.get(i).copied();

    assert!(backend.option_popup_row_rect("T", &get, 2, 1).is_some());
    assert!(backend.option_popup_row_rect("T", &get, 2, 9).is_none());
}

#[test]
fn a_dialog_row_rect_comes_back_as_written() {
    let backend = backend();
    let get = |i: usize| ["A", "B", "C"].get(i).copied();

    let first = backend.option_popup_row_rect("T", &get, 3, 0).unwrap();
    let second = backend.option_popup_row_rect("T", &get, 3, 1).unwrap();

    assert_eq!(first.width(), testing::WIDTH - 32);
    assert!(
        second.y() > first.y(),
        "rows stack downwards: {first:?} then {second:?}"
    );
}

// -- fonts -----------------------------------------------------------------

/// A build that compiled a font out reports id 0. The framework then measures
/// zero and expects nothing to be painted — a backend that crossed the
/// boundary anyway would draw at whatever the C++ side substituted.
#[test]
fn an_unavailable_font_draws_nothing_and_measures_zero() {
    let backend = backend();
    let missing = FontId::UNAVAILABLE;

    backend.draw_text(Point::new(10, 10), "Hello", missing, FontStyle::Regular);
    assert!(
        testing::calls().is_empty(),
        "nothing crossed the boundary: {:?}",
        testing::calls()
    );

    assert_eq!(backend.text_width(missing, "Hello", FontStyle::Regular), 0);
    assert_eq!(backend.line_height(missing), 0);
}

#[test]
fn each_font_role_crosses_as_its_own_tag() {
    let backend = backend();
    let ui = backend.font(FontRole::Ui);
    let small = backend.font(FontRole::UiSmall);
    let reader = backend.font(FontRole::Reader);

    assert!(ui.is_available() && small.is_available() && reader.is_available());
    assert_ne!(ui, small);
    assert_ne!(small, reader);
}

#[test]
fn a_style_crosses_as_its_own_number() {
    let backend = backend();
    let font = backend.font(FontRole::Ui);
    backend.draw_text(Point::new(0, 0), "x", font, FontStyle::BoldItalic);

    let Some(Call::Text { style, .. }) = testing::calls().into_iter().next() else {
        panic!("nothing was drawn");
    };
    assert_eq!(style, FontStyle::BoldItalic as u8);
}

// -- canvas ----------------------------------------------------------------

/// `None` means "lift the clip", and the ABI spells that as a zero-sized rect
/// rather than needing a second symbol.
#[test]
fn clearing_the_clip_crosses_as_a_zero_sized_rect() {
    let backend = backend();
    backend.set_clip(Some(Rect::new(1, 2, 30, 40)));
    backend.set_clip(None);

    assert_eq!(
        testing::calls(),
        vec![Call::Clip { w: 30, h: 40 }, Call::Clip { w: 0, h: 0 }]
    );
}

/// The C++ side reads a fixed number of bytes from the pointer it is given, so
/// a slice shorter than the bitmap it claims to be would walk off the end of
/// Rust's allocation.
#[test]
fn a_bitmap_shorter_than_its_own_size_is_refused() {
    let backend = backend();

    // 16x2 needs 2 bytes per row, so 4. Give it 3.
    backend.draw_image(Point::new(0, 0), &[0, 0, 0], Size::new(16, 2));
    assert!(
        testing::calls().is_empty(),
        "a short buffer must not cross: {:?}",
        testing::calls()
    );

    backend.draw_image(Point::new(0, 0), &[0, 0, 0, 0], Size::new(16, 2));
    assert_eq!(testing::calls(), vec![Call::Image { w: 16, h: 2 }]);
}

#[test]
fn an_empty_bitmap_is_refused() {
    let backend = backend();
    backend.draw_image(Point::new(0, 0), &[], Size::new(0, 0));
    backend.draw_image(Point::new(0, 0), &[1, 2], Size::new(-4, 4));
    assert!(testing::calls().is_empty());
}

#[test]
fn ink_and_background_cross_as_different_flags() {
    let backend = backend();
    backend.fill_rect(Rect::new(0, 0, 10, 10), true);
    backend.fill_rect(Rect::new(0, 0, 10, 10), false);

    let flags: Vec<bool> = testing::calls()
        .into_iter()
        .filter_map(|call| match call {
            Call::Rect { black, .. } => Some(black),
            _ => None,
        })
        .collect();
    assert_eq!(flags, vec![true, false]);
}

/// The scrim and the dither are separate symbols because they are separate
/// operations — one preserves what is behind it and the other does not.
#[test]
fn a_scrim_and_a_dither_are_not_the_same_call() {
    let backend = backend();
    backend.scrim(Rect::new(0, 0, 10, 10));
    backend.fill_rect_dither(Rect::new(0, 0, 10, 10), true);

    assert_eq!(
        testing::calls(),
        vec![Call::Scrim, Call::Dither { light: true }]
    );
}

#[test]
fn an_icon_carries_its_role_and_size() {
    let backend = backend();
    let icon = IconRef {
        kind: 7,
        variant: 1,
        size: 32,
    };
    backend.draw_icon(Point::new(4, 5), icon);
    assert_eq!(testing::calls(), vec![Call::Icon { role: 7, size: 32 }]);
    assert_eq!(backend.icon_size(icon), 32);
}

// -- metrics ---------------------------------------------------------------

/// The tag numbers are positional and NOT in declaration order — `ListRowGap`
/// is 14, sitting after the slider values. Every one must round-trip, or a
/// layout measures against the wrong number and nothing looks obviously wrong.
#[test]
fn every_metric_crosses_as_its_own_tag() {
    use xpui::host::ThemeMetric::*;
    let backend = backend();

    let all = [
        (TopPadding, 8),
        (HeaderHeight, 40),
        (VerticalSpacing, 12),
        (ButtonHintsHeight, 38),
        (ContentSidePadding, 16),
        (ContentTop, 60),
        (ContentBottom, 760),
        (ListRowHeight, 44),
        (ListRowHeightWithSubtitle, 56),
        (ProgressBarHeight, 6),
        (MinTouchSize, 48),
        (SliderKnobWidth, 14),
        (SliderKnobHeight, 22),
        (SliderSideInset, 10),
        (ListRowGap, 4),
        (SubHeaderHeight, 17),
        (SpacingSmall, 5),
    ];

    // Distinct on purpose. If two metrics answered the same number, a swap
    // between them would sail through the loop below — and the tags are
    // positional, so a swap is exactly the mistake to expect.
    let mut seen: Vec<i32> = all.iter().map(|(_, value)| *value).collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(
        seen.len(),
        before,
        "two metrics share a value, so this test could not catch a swapped tag"
    );

    for (metric, expected) in all {
        assert_eq!(
            backend.metric(metric),
            expected,
            "{metric:?} came back as the wrong tag's value"
        );
    }
}

/// The framework converts a touch into a slider value using these three, so
/// they have to be the numbers the shim actually paints with.
#[test]
fn the_slider_metrics_are_the_ones_a_touch_is_measured_against() {
    use xpui::host::ThemeMetric::*;
    let backend = backend();

    assert!(backend.metric(SliderKnobWidth) > 0);
    assert!(backend.metric(SliderSideInset) > 0);
    assert!(
        backend.metric(SliderKnobWidth) < backend.metric(MinTouchSize),
        "a knob wider than the minimum touch target has nowhere to travel"
    );
}

// -- the whole host --------------------------------------------------------

/// Installing it must satisfy every trait. This is the assertion that catches
/// a method added to the framework and not forwarded here.
#[test]
fn the_backend_is_a_complete_host() {
    fn assert_host<T: xpui::host::Host>() {}
    assert_host::<Backend<NoInput>>();
}

#[test]
fn a_repaint_request_reaches_the_panel() {
    let backend = backend();
    backend.request_update();
    assert_eq!(testing::calls(), vec![Call::RequestUpdate]);
}
