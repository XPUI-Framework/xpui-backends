// The xpui side of FreeInkUI.
//
// Every symbol in xpui_fui.h, drawn through freeink::ui::DisplayTarget: a raw
// 1-bit framebuffer, the SDK's bundled Noto Sans bitmap font, and the SDK's own
// components. Nothing here names a product, so any firmware that can hand over
// a framebuffer can host xpui.
//
// Three conventions are decided once, here, because they are what the file is
// for:
//
//  * Ink. A SET bit is WHITE — xpui_fui_attach's contract, and FreeInkDisplay's
//    own. DisplayTarget already works that way, so ink is Color::Black,
//    background is Color::White, and nothing inverts on the way through. The
//    exception is xpui_fui_draw_image, whose bitmaps have BIT 0 = INK: that is
//    precisely FreeInkUI's BitmapFormat::Mask1, so such a bitmap goes in as
//    Mask1 and forEachBitmapPixel folds the polarity in — no copy, no flip.
//
//  * Coordinates. xpui draws in logical coordinates and the buffer it hands
//    over is already in that frame ((width + 7) / 8 bytes per row), so the
//    DisplayTarget is built in its native orientation and logical -> panel is
//    the identity. A firmware that rotates its panel rotates it on the way to
//    the glass, not here.
//
//  * Font ids. xpui_fui_font must be able to say "this build ships no such
//    font" by returning 0, but 0 is also FreeInkUI's FONT_SLOT_SMALL. So the
//    ids this file hands out are FUI slot + 1, and 0 stays reserved for "none".
//    Every entry point that takes a font id decodes it through slotFor().
//
// The shim is stateless apart from the framebuffer and the clip: each call
// builds its own DisplayTarget, Frame and props and retains nothing. It is not
// reentrant — one framebuffer, one clip, one scratch row array.

#include "xpui_fui.h"

#include <FreeInkApp.h>
#include <FreeInkUI.h>
#include <FreeInkUIDisplayTarget.h>
#include <string.h>

namespace {

namespace fui = freeink::ui;

// -- panel -------------------------------------------------------------------

uint8_t* g_framebuffer = nullptr;
int32_t g_width = 0;
int32_t g_height = 0;
int32_t g_stride = 0;  // bytes per framebuffer row

// The clip, in screen coordinates. Zero width or height means "no clip".
fui::Rect g_clip{};

// Derived from the bound font's line height once, then reused: rowHeight,
// header/footer bands and the touch minimum all scale with the face, and
// xpui_fui_metric has to report the same numbers the components paint with.
fui::ThemeTokens g_theme;
bool g_themeReady = false;

// The reading face gets a slot of its own so a firmware can rebind it without
// disturbing the UI slots. All four default to the bundled font.
constexpr fui::FontId SLOT_READING = 3;

// One repaint of a list only ever shows the rows that fit the band, so the
// window is bounded and can live in BSS instead of on the stack or the heap.
// 24 rows is past what any e-ink panel fits at a legible row height.
constexpr int32_t MAX_LIST_ROWS = 24;
fui::ListItem g_listItems[MAX_LIST_ROWS];

// An option dialog does not scroll, so options past this would render off the
// panel anyway and a fixed cap keeps the array on the stack.
constexpr int32_t MAX_DIALOG_OPTIONS = 16;

bool attached() { return g_framebuffer != nullptr && g_width > 0 && g_height > 0; }

const char* asText(const uint8_t* text) { return reinterpret_cast<const char*>(text); }

int32_t clampInt(const int32_t value, const int32_t low, const int32_t high) {
  return value < low ? low : (value > high ? high : value);
}

// The theme is pure font metrics, so it answers before a framebuffer is bound —
// xpui asks for metrics while it is still laying a screen out.
const fui::ThemeTokens& theme() {
  if (!g_themeReady) {
    const fui::DisplayTarget probe(nullptr, 0, 0, 0);  // metrics only; never draws
    g_theme = fui::themeTokensForLineHeight(probe.lineHeight(fui::FONT_SLOT_BODY), fui::FONT_SLOT_SMALL,
                                            fui::FONT_SLOT_BODY, fui::FONT_SLOT_TITLE);
    g_themeReady = true;
  }
  return g_theme;
}

// -- the clip ----------------------------------------------------------------
//
// FreeInkUI has no clipping, so the clip is enforced by the framebuffer VIEW
// rather than by intercepting draw calls. The windowed DisplayTarget starts at
// the clip's first row and its logical frame ends at the clip's last row and
// right edge, so DisplayTarget::plot's own bounds check drops everything above,
// below and to the right — glyph pixels included, exactly, for free.
//
// The left edge is the one that cannot be said that way: moving the row pointer
// sideways only moves in whole bytes, so it would snap to a multiple of 8. The
// primitives below intersect their own rects against it instead, which is exact
// for fills and bitmaps; text is confined to the rect it was given.

bool clipping() { return g_clip.width > 0 && g_clip.height > 0; }

int16_t clipTop() { return clipping() ? g_clip.y : 0; }

int16_t clipWindowHeight() { return clipping() ? g_clip.height : fui::clampI16(g_height); }

// The window's logical width IS the clip's right edge: x is never translated,
// so plot's `x >= w_` test lands on the exact pixel column.
int16_t clipWindowWidth() { return clipping() ? g_clip.right() : fui::clampI16(g_width); }

// A DisplayTarget windowed to the current clip, plus the screen -> window
// translation that goes with it. Built per call; it owns nothing.
class Canvas {
 public:
  Canvas()
      : originY_(clipTop()),
        target_(g_framebuffer + static_cast<int32_t>(clipTop()) * g_stride, clipWindowWidth(), clipWindowHeight(),
                fui::clampI16(g_stride), fui::Orientation::LandscapeCounterClockwise) {}

  fui::DisplayTarget& target() { return target_; }
  const fui::DisplayTarget& target() const { return target_; }

  // Screen coordinates in, window coordinates out.
  fui::Rect place(const int32_t x, const int32_t y, const int32_t w, const int32_t h) const {
    return fui::makeRect(x, y - originY_, w, h);
  }
  fui::Rect place(const fui::Rect rect) const { return place(rect.x, rect.y, rect.width, rect.height); }

  // The left edge of the clip, which the window cannot express. Exact for
  // anything that fills a rect; empty when the rect falls outside entirely.
  fui::Rect confine(fui::Rect rect) const {
    if (!clipping()) return rect;
    const int16_t left = g_clip.x > rect.x ? g_clip.x : rect.x;
    const int16_t right = g_clip.right() < rect.right() ? g_clip.right() : rect.right();
    if (right <= left) return fui::Rect{};
    rect.x = left;
    rect.width = static_cast<int16_t>(right - left);
    return rect;
  }

  // Sized to the window, so a component's screen() and safeRect() agree with
  // what actually reaches the panel.
  fui::DeviceContext device() const {
    fui::DeviceContext device;
    device.width = target_.logicalWidth();
    device.height = target_.logicalHeight();
    device.orientation = target_.orientation();
    device.touchOrientation = target_.touchOrientation();
    device.hasButtons = true;
    device.minTouchSize = theme().minTouchSize;
    return device;
  }

 private:
  int16_t originY_;
  fui::DisplayTarget target_;
};

// -- fonts -------------------------------------------------------------------

// Decodes an ABI font id back to a FreeInkUI slot. Returns false for 0, the
// reserved "this build ships no such font" answer.
bool slotFor(const int32_t fontId, fui::FontId& slot) {
  if (fontId <= 0 || fontId > fui::DisplayTarget::FONT_SLOTS) return false;
  slot = static_cast<fui::FontId>(fontId - 1);
  return true;
}

// The bundled font has one weight, so `bold` only reaches a firmware that has
// bound a real family to the slot; italic has nowhere to go at all.
fui::TextStyle textStyle(const fui::FontId slot, const uint8_t style) {
  fui::TextStyle text;
  text.font = slot;
  text.bold = (style & 0x01) != 0;
  return text;
}

// -- dither ------------------------------------------------------------------

// A 2x2 checkerboard, tiled: 50% ink, which is neither of DisplayTarget's Bayer
// grays (LightGray is 25%, DarkGray 75%). `parity` is the SCREEN parity that
// takes ink, so a pattern stays put no matter which rect asks for it — two
// bands drawn at the same parity line up seam-free.
fui::BitmapRef checkerboard(const fui::Rect rect, const int parity) {
  static constexpr uint8_t kEven[] = {0x80, 0x40};  // ink where (x + y) is even
  static constexpr uint8_t kOdd[] = {0x40, 0x80};
  // Tiling is anchored to the rect, not the screen, so fold the rect's own
  // offset into the choice of pattern.
  const bool even = ((rect.x + rect.y + parity) & 1) == 0;
  return fui::BitmapRef{even ? kEven : kOdd, 2, 2, fui::BitmapFormat::BW1};
}

void tileCheckerboard(fui::DisplayTarget& target, const fui::Rect rect, const int parity) {
  if (rect.empty()) return;
  target.fill(rect, fui::Paint::bitmapFill(fui::BitmapFill{checkerboard(rect, parity), fui::BitmapMode::Tile}));
}

// -- chrome geometry ---------------------------------------------------------
//
// The bands xpui_fui_metric describes and the ones drawn below are the same
// bands: both read these.

int32_t topPadding() { return 0; }
int32_t headerHeight() { return theme().headerHeight; }
int32_t verticalSpacing() { return theme().spaceMd; }
int32_t buttonHintsHeight() { return theme().footerHeight; }
int32_t contentSidePadding() { return theme().spaceMd; }

// A list insets its own text within the band, so the header's title starts at
// the same column as a row label instead of a few pixels to its left.
int32_t headerSidePadding() { return contentSidePadding() + theme().listSidePadding; }

// -- option dialog -----------------------------------------------------------

// The painter and the hit-tester must agree to the pixel, and the hit-tester
// runs before the first paint, so both build the dialog from this one function
// and neither reads anything back. The panel's size depends only on whether
// there is a title and on how many options there are — never on their text —
// which is exactly what xpui_fui_option_popup_row_rect is handed.
struct PopupLayout {
  fui::Rect panel{};  // screen coordinates
  fui::OptionDialogProps props{};
  int32_t shown = 0;
};

// `options` is the array the dialog will be painted from. The layout only needs
// it to exist — optionDialogHeight reserves a row per optionCount and never
// reads a label — so the hit-tester passes an empty one and the painter fills
// its own in afterwards.
bool popupLayout(const fui::DisplayTarget& target, const uint8_t* title, const int32_t count,
                 const fui::DialogOption* options, PopupLayout& out) {
  if (count <= 0 || !options) return false;
  const fui::ThemeTokens& tokens = theme();

  out.shown = count < MAX_DIALOG_OPTIONS ? count : MAX_DIALOG_OPTIONS;
  out.props.title = title ? asText(title) : nullptr;
  out.props.options = options;
  out.props.optionCount = static_cast<uint8_t>(out.shown);
  out.props.verticalOptions = true;
  out.props.padding = fui::Insets{tokens.spaceMd, tokens.spaceLg, tokens.spaceMd, tokens.spaceLg};
  out.props.gap = tokens.spaceSm;
  out.props.buttonHeight = fui::clampI16(target.lineHeight(tokens.bodyText.font) + tokens.spaceMd * 2);
  out.props.titleText = tokens.titleText;
  out.props.titleText.align = fui::TextAlign::Center;
  out.props.buttonText = tokens.bodyText;
  out.props.buttonStyles = tokens.button;
  out.props.styles = tokens.popup;
  // defaultPopupStyles is a white panel with no outline, which on a white page
  // is no panel at all. xpui dims behind the dialog with its own scrim; the
  // border is what gives the panel an edge.
  out.props.styles.normal.border = fui::Paint::solid(fui::Color::Black);
  out.props.styles.normal.borderWidth = 1;

  const fui::Rect screen{0, 0, fui::clampI16(g_width), fui::clampI16(g_height)};
  const int32_t sideMargin = tokens.spaceLg * 2;
  const int16_t width = fui::clampI16(screen.width * 3 / 4 < screen.width - sideMargin ? screen.width * 3 / 4
                                                                                       : screen.width - sideMargin);
  if (width <= 0) return false;
  const int16_t height = fui::clampI16(fui::optionDialogHeight(target, out.props, width), 0, screen.height);
  out.panel = fui::centeredRect(screen, fui::Size{width, height});
  return !out.panel.empty();
}

// optionDialog's own vertical-option geometry, echoed for the hit-tester off
// the same panel rect and the same props the painter is handed.
bool popupRowRect(const PopupLayout& layout, const int32_t index, fui::Rect& out) {
  if (index < 0 || index >= layout.shown) return false;
  const fui::Rect content = layout.panel.inset(layout.props.padding);
  const int16_t step = static_cast<int16_t>(layout.props.buttonHeight + layout.props.gap);
  const int16_t buttonsH =
      static_cast<int16_t>(layout.shown * layout.props.buttonHeight + (layout.shown - 1) * layout.props.gap);
  out = fui::Rect{content.x, static_cast<int16_t>(content.bottom() - buttonsH + index * step), content.width,
                  layout.props.buttonHeight};
  return true;
}

// -- button hints ------------------------------------------------------------

// A hint slot. NULL asks for the shim's own label; an EMPTY STRING asks for a
// blank slot. Resolving both to "" here is the bug this function exists to
// prevent — it would silently blank every slot a screen did not name.
const char* hintLabel(const uint8_t* label, const char* standard) { return label ? asText(label) : standard; }

}  // namespace

extern "C" {

// -- lifecycle ---------------------------------------------------------------

void xpui_fui_attach(uint8_t* framebuffer, const int32_t width, const int32_t height) {
  g_framebuffer = framebuffer;
  g_width = width;
  g_height = height;
  g_stride = (width + 7) / 8;
  g_clip = fui::Rect{};
}

// -- canvas ------------------------------------------------------------------

int32_t xpui_fui_screen_width(void) { return g_width; }
int32_t xpui_fui_screen_height(void) { return g_height; }

void xpui_fui_clear(void) {
  if (!attached()) return;
  // Straight at the buffer: background is every bit set, and the clip does not
  // apply to a clear.
  memset(g_framebuffer, 0xFF, static_cast<size_t>(g_stride) * static_cast<size_t>(g_height));
}

void xpui_fui_draw_text(const int32_t x, const int32_t y, const uint8_t* text, const int32_t fontId,
                        const uint8_t style) {
  fui::FontId slot;
  if (!attached() || !text || !slotFor(fontId, slot)) return;

  Canvas canvas;
  fui::TextStyle textStyle_ = textStyle(slot, style);
  // From the top-left corner, not the baseline: layoutText only centers a block
  // that is shorter than its rect, so a rect exactly one line tall top-aligns.
  // The width is the measured width so the run neither wraps nor ellipsizes.
  const fui::Size size = canvas.target().measureText(slot, asText(text), textStyle_);
  canvas.target().text(canvas.place(x, y, size.width, canvas.target().lineHeight(slot)), asText(text), textStyle_);
}

void xpui_fui_fill_rect(const int32_t x, const int32_t y, const int32_t w, const int32_t h, const uint8_t black) {
  if (!attached()) return;
  Canvas canvas;
  canvas.target().fill(canvas.confine(canvas.place(x, y, w, h)),
                       fui::Paint::solid(black ? fui::Color::Black : fui::Color::White));
}

void xpui_fui_stroke_rect(const int32_t x, const int32_t y, const int32_t w, const int32_t h) {
  if (!attached()) return;
  Canvas canvas;
  // Not confined: intersecting a stroke would move its edges inward rather than
  // cut them. The window still clips it above, below and right.
  canvas.target().stroke(canvas.place(x, y, w, h), fui::Paint::solid(fui::Color::Black), 1);
}

void xpui_fui_draw_line(const int32_t x1, const int32_t y1, const int32_t x2, const int32_t y2) {
  if (!attached()) return;
  Canvas canvas;
  const fui::Rect from = canvas.place(x1, y1, 0, 0);
  const fui::Rect to = canvas.place(x2, y2, 0, 0);
  canvas.target().line(fui::Point{from.x, from.y}, fui::Point{to.x, to.y}, 1, fui::Paint::solid(fui::Color::Black));
}

void xpui_fui_fill_rect_dither(const int32_t x, const int32_t y, const int32_t w, const int32_t h,
                               const uint8_t light) {
  if (!attached()) return;
  Canvas canvas;
  const fui::Rect rect = canvas.confine(canvas.place(x, y, w, h));
  if (rect.empty()) return;
  // Clear first: a dither is a grey FILL, and whatever was under it is gone.
  canvas.target().fill(rect, fui::Paint::solid(fui::Color::White));
  tileCheckerboard(canvas.target(), rect, light ? 1 : 0);
}

void xpui_fui_scrim(const int32_t x, const int32_t y, const int32_t w, const int32_t h) {
  if (!attached()) return;
  Canvas canvas;
  // No clear, deliberately: half the pixels keep whatever was there, so the
  // region greys down with its content still readable underneath.
  tileCheckerboard(canvas.target(), canvas.confine(canvas.place(x, y, w, h)), 0);
}

void xpui_fui_set_clip(const int32_t x, const int32_t y, const int32_t w, const int32_t h) {
  if (w <= 0 || h <= 0 || !attached()) {
    g_clip = fui::Rect{};
    return;
  }
  const int32_t left = clampInt(x, 0, g_width);
  const int32_t top = clampInt(y, 0, g_height);
  const int32_t right = clampInt(x + w, left, g_width);
  const int32_t bottom = clampInt(y + h, top, g_height);
  g_clip = fui::makeRect(left, top, right - left, bottom - top);
}

void xpui_fui_draw_image(const uint8_t* bitmap, const int32_t x, const int32_t y, const int32_t w, const int32_t h) {
  if (!attached() || !bitmap || w <= 0 || h <= 0) return;
  Canvas canvas;
  // BIT 0 = INK is FreeInkUI's Mask1, so the polarity is handled by the sampler
  // rather than by inverting a copy of the caller's rows.
  const fui::BitmapRef image{bitmap, static_cast<uint16_t>(clampInt(w, 0, 65535)),
                             static_cast<uint16_t>(clampInt(h, 0, 65535)), fui::BitmapFormat::Mask1};
  // Center at a rect of the bitmap's own size draws it 1:1 at (x, y).
  canvas.target().bitmap(canvas.confine(canvas.place(x, y, w, h)), image, fui::BitmapMode::Center,
                         fui::Paint::solid(fui::Color::Black));
}

void xpui_fui_draw_icon(const uint16_t role, const uint8_t variant, const int32_t size, const int32_t x,
                        const int32_t y) {
  // This build ships no icon set — see xpui_fui_icon_size. Drawing a
  // substitute glyph would occupy space the caller was told to leave out.
  (void)role;
  (void)variant;
  (void)size;
  (void)x;
  (void)y;
}

int32_t xpui_fui_icon_size(const uint16_t role, const uint8_t variant, const int32_t size) {
  // DisplayTarget ships a font, not an icon set: the Icons library is a
  // separate opt-in (FreeInkUIIcon.h). 0 tells the caller to reserve nothing.
  (void)role;
  (void)variant;
  (void)size;
  return 0;
}

// -- fonts -------------------------------------------------------------------

int32_t xpui_fui_font(const uint8_t role) {
  // Ids are FUI slot + 1 so that 0 can keep meaning "no such font".
  switch (role) {
    case 0:
      return fui::FONT_SLOT_BODY + 1;
    case 1:
      return fui::FONT_SLOT_SMALL + 1;
    case 2:
      return SLOT_READING + 1;
    default:
      return 0;
  }
}

int32_t xpui_fui_text_width(const int32_t fontId, const uint8_t* text, const uint8_t style) {
  fui::FontId slot;
  if (!text || !slotFor(fontId, slot)) return 0;
  const fui::DisplayTarget probe(nullptr, 0, 0, 0);  // measuring needs no framebuffer
  return probe.measureText(slot, asText(text), textStyle(slot, style)).width;
}

int32_t xpui_fui_line_height(const int32_t fontId) {
  fui::FontId slot;
  if (!slotFor(fontId, slot)) return 0;
  const fui::DisplayTarget probe(nullptr, 0, 0, 0);
  return probe.lineHeight(slot);
}

// -- theme -------------------------------------------------------------------

int32_t xpui_fui_metric(const uint8_t metric) {
  // Mirrors xpui's ThemeMetric. The tag numbers are NOT the declaration order —
  // ListRowGap is 14, after SliderSideInset and before SubHeaderHeight — so
  // they are spelled out rather than left to the enum.
  enum class Metric : uint8_t {
    TopPadding = 0,
    HeaderHeight = 1,
    VerticalSpacing = 2,
    ButtonHintsHeight = 3,
    ContentSidePadding = 4,
    ContentTop = 5,
    ContentBottom = 6,
    ListRowHeight = 7,
    ListRowHeightWithSubtitle = 8,
    ProgressBarHeight = 9,
    MinTouchSize = 10,
    SliderKnobWidth = 11,
    SliderKnobHeight = 12,
    SliderSideInset = 13,
    ListRowGap = 14,
    SubHeaderHeight = 15,
    SpacingSmall = 16,
  };

  const fui::ThemeTokens& tokens = theme();
  const fui::DisplayTarget probe(nullptr, 0, 0, 0);  // font metrics only

  switch (static_cast<Metric>(metric)) {
    case Metric::TopPadding:
      return topPadding();
    case Metric::HeaderHeight:
      return headerHeight();
    case Metric::VerticalSpacing:
      return verticalSpacing();
    case Metric::ButtonHintsHeight:
      return buttonHintsHeight();
    case Metric::ContentSidePadding:
      return contentSidePadding();
    case Metric::ContentTop:
      return topPadding() + headerHeight() + verticalSpacing();
    case Metric::ContentBottom:
      // The same gap ContentTop leaves under the header, so a scrolling view
      // stops short of the hints instead of ending flush against them.
      return attached() ? g_height - buttonHintsHeight() - verticalSpacing() : 0;
    case Metric::ListRowHeight:
      // The theme's row height, which is what Screen::list lays a list out with
      // when rowHeight is left unset — as xpui_fui_draw_list leaves it.
      return tokens.rowHeight;
    case Metric::ListRowHeightWithSubtitle:
      // A second line needs the room for it, measured in the same font
      // Screen::list substitutes for an unset subtitle style.
      return tokens.rowHeight + probe.lineHeight(tokens.smallText.font);
    case Metric::ProgressBarHeight:
      return tokens.progressHeight;
    case Metric::MinTouchSize:
      return tokens.minTouchSize;
    // Read straight off FreeInkUI's defaults rather than repeated here, so the
    // slider a touch is converted against is the slider that gets painted.
    case Metric::SliderKnobWidth:
      return fui::SliderProps{}.knobWidth;
    case Metric::SliderKnobHeight:
      return fui::SliderProps{}.knobHeight;
    case Metric::SliderSideInset:
      return fui::SliderProps{}.horizontalPadding;
    case Metric::ListRowGap:
      return tokens.listRowGap;
    case Metric::SubHeaderHeight:
      // xpui_fui_draw_sub_header draws one top-aligned line and ignores the
      // band's height, so the band is exactly that line.
      return probe.lineHeight(tokens.bodyText.font);
    case Metric::SpacingSmall:
      return tokens.spaceSm;
  }
  return 0;
}

void xpui_fui_draw_header(const uint8_t* title, const uint8_t* subtitle) {
  if (!attached()) return;
  const fui::ThemeTokens& tokens = theme();

  Canvas canvas;
  const fui::DeviceContext device = canvas.device();  // Frame holds this by reference
  const fui::InputSnapshot noInput{};
  fui::InteractionBuffer<1> hits;
  fui::Frame<1> frame(canvas.target(), device, noInput, hits);

  fui::HeaderProps props;
  props.title = title ? asText(title) : nullptr;
  props.subtitle = subtitle ? asText(subtitle) : nullptr;
  props.titleText = tokens.titleText;
  props.titleText.align = tokens.headerTitleAlign;
  props.subtitleText = tokens.smallText;
  props.styles = tokens.popup;
  props.borderEdges = fui::EdgeBottom;
  if (tokens.headerUnderline > 0) {
    props.styles.normal.border = fui::Paint::solid(fui::Color::Black);
    props.styles.normal.borderWidth = tokens.headerUnderline;
  }
  props.sidePadding = fui::clampI16(headerSidePadding());
  props.minTouchSize = tokens.minTouchSize;

  fui::header(frame, canvas.place(0, topPadding(), g_width, headerHeight()), props);
}

void xpui_fui_draw_sub_header(const int32_t x, const int32_t y, const int32_t w, const int32_t h, const uint8_t* label,
                              const uint8_t* right) {
  if (!attached() || !label) return;
  const fui::ThemeTokens& tokens = theme();

  Canvas canvas;
  // FreeInkUI has no sub-header component: a group heading is a bold body line
  // with an optional value opposite it. The band's height is the caller's
  // business — the line is drawn at its top, which is what SubHeaderHeight
  // reports.
  fui::TextStyle labelStyle = tokens.bodyText;
  labelStyle.bold = true;
  const int16_t lineHeight = canvas.target().lineHeight(labelStyle.font);
  (void)h;

  fui::Rect line = canvas.place(x, y, w, lineHeight);
  if (right) {
    fui::TextStyle rightStyle = tokens.smallText;
    rightStyle.align = fui::TextAlign::Right;
    const int16_t rightWidth = canvas.target().measureText(rightStyle.font, asText(right), rightStyle).width;
    canvas.target().text(fui::Rect{static_cast<int16_t>(line.right() - rightWidth), line.y, rightWidth, lineHeight},
                         asText(right), rightStyle);
    line.width = fui::clampI16(line.width - rightWidth - tokens.spaceSm);
  }
  canvas.target().text(line, asText(label), labelStyle);
}

void xpui_fui_draw_button_hints(const uint8_t* back, const uint8_t* confirm, const uint8_t* previous,
                                const uint8_t* next) {
  if (!attached()) return;
  const fui::ThemeTokens& tokens = theme();

  // No input manager reaches this far down, so the slots are drawn in the order
  // the ABI names them. A firmware that lets the user remap its front buttons
  // reorders the four arguments on the way in.
  const char* labels[4] = {
      hintLabel(back, "Back"),
      hintLabel(confirm, "Select"),
      hintLabel(previous, "Up"),
      hintLabel(next, "Down"),
  };

  Canvas canvas;
  const fui::Rect band = canvas.place(0, g_height - buttonHintsHeight(), g_width, buttonHintsHeight());
  if (band.empty()) return;
  canvas.target().fill(band, fui::Paint::solid(fui::Color::White));
  canvas.target().fill(fui::Rect{band.x, band.y, band.width, 1}, fui::Paint::solid(fui::Color::Black));

  const fui::Rect content =
      fui::Rect{band.x, static_cast<int16_t>(band.y + 1), band.width, static_cast<int16_t>(band.height - 1)}.inset(
          fui::Insets{0, tokens.spaceSm, 0, tokens.spaceSm});
  if (content.empty()) return;

  fui::TextStyle hintStyle = tokens.smallText;
  hintStyle.align = fui::TextAlign::Center;
  const int16_t slotWidth = static_cast<int16_t>(content.width / 4);
  for (int i = 0; i < 4; ++i) {
    if (!labels[i] || labels[i][0] == '\0') continue;  // an empty label is a blank slot
    // The last slot takes the division's remainder so the four fill the band.
    const int16_t left = static_cast<int16_t>(content.x + i * slotWidth);
    const int16_t width = i == 3 ? static_cast<int16_t>(content.right() - left) : slotWidth;
    canvas.target().text(fui::Rect{left, content.y, width, content.height}, labels[i], hintStyle);
  }
}

void xpui_fui_draw_progress_bar(const int32_t x, const int32_t y, const int32_t w, const int32_t h,
                                const uint32_t current, const uint32_t total) {
  if (!attached()) return;

  Canvas canvas;
  const fui::DeviceContext device = canvas.device();
  const fui::InputSnapshot noInput{};
  fui::InteractionBuffer<1> hits;
  fui::Frame<1> frame(canvas.target(), device, noInput, hits);

  fui::ProgressBarProps props;
  props.value = static_cast<int32_t>(current > 0x7FFFFFFFu ? 0x7FFFFFFFu : current);
  props.max = static_cast<int32_t>(total > 0x7FFFFFFFu ? 0x7FFFFFFFu : total);
  // An empty bar still has to read as a bar, so the track is a light dither
  // rather than the component's transparent default.
  props.track = fui::Paint::dither(fui::Color::LightGray);
  fui::progressBar(frame, canvas.place(x, y, w, h), props);
}

void xpui_fui_draw_slider(const int32_t x, const int32_t y, const int32_t w, const int32_t h, const int32_t value,
                          const int32_t max) {
  if (!attached()) return;

  Canvas canvas;
  const fui::DeviceContext device = canvas.device();
  const fui::InputSnapshot noInput{};
  fui::InteractionBuffer<1> hits;
  fui::Frame<1> frame(canvas.target(), device, noInput, hits);

  fui::SliderProps props;
  props.value = value;
  props.max = max;
  // xpui declared this slider's touch region and routes the drag itself, so the
  // component only draws. Everything else stays at the defaults xpui_fui_metric
  // reports back for SliderKnob*/SliderSideInset.
  props.action = fui::NO_ACTION;
  fui::slider(frame, canvas.place(x, y, w, h), props);
}

void xpui_fui_draw_scroll_indicator(const int32_t x, const int32_t y, const int32_t w, const int32_t h,
                                    const int32_t content, const int32_t visible, const int32_t offset) {
  if (!attached() || content <= 0 || visible <= 0) return;

  Canvas canvas;
  const fui::ThemeTokens& tokens = theme();
  // The theme sets the track's thickness; the rect says where it may live. It
  // draws nothing at all when content <= visible, which is the contract.
  const int16_t width = tokens.listScrollWidth < w ? tokens.listScrollWidth : fui::clampI16(w);
  fui::drawListScrollIndicator(canvas.target(), canvas.place(x, y, w, h), static_cast<uint32_t>(content),
                               static_cast<uint32_t>(visible), static_cast<uint32_t>(offset < 0 ? 0 : offset), width,
                               tokens.listScrollSide, tokens.listScrollInset);
}

void xpui_fui_draw_list(const int32_t x, const int32_t y, const int32_t w, const int32_t h, const int32_t rows,
                        const int32_t selected, const xpui_fui_cell_fn cell, void* ctx) {
  if (!attached() || !cell || rows <= 0) return;

  Canvas canvas;
  const fui::ThemeTokens& tokens = theme();
  const fui::Rect band = canvas.place(x, y, w, h);
  if (band.empty()) return;

  const int16_t plainRow = tokens.rowHeight;
  const int16_t subtitledRow = fui::clampI16(plainRow + canvas.target().lineHeight(tokens.smallText.font));

  // The window and the row height decide each other — a subtitle needs a second
  // line, and taller rows mean fewer of them — so measure, fill, and if a
  // subtitle turned up, measure and fill once more. It settles in two passes.
  int16_t rowHeight = plainRow;
  int32_t visible = 0;
  int32_t top = 0;
  int32_t shown = 0;
  for (int pass = 0; pass < 2; ++pass) {
    visible = fui::listVisibleRows(band, rowHeight, tokens.listRowGap);
    if (visible > MAX_LIST_ROWS) visible = MAX_LIST_ROWS;
    // Not even one whole row fits: the component would refuse to draw a partial
    // row anyway, and a scroll indicator beside nothing would be a lie.
    if (visible < 1) return;
    // Deliberately NOT scrolled to keep `selected` in view, tempting as that
    // is. xpui owns scrolling: a `ScrollView` translates this rect's origin,
    // and `List::interactions` declares its touch rects as row 0 at the top of
    // the rect it was given. A shim that scrolled independently would paint
    // row N where the framework registered row 0 — every tap off by the scroll
    // distance, and nothing about the screen looking wrong.
    top = 0;
    shown = rows - top < visible ? rows - top : visible;
    if (shown <= 0) return;

    bool anySubtitle = false;
    for (int32_t i = 0; i < shown; ++i) {
      // The strings live in xpui's own buffers for the length of this call, so
      // the items point straight at them rather than copying a row at a time.
      fui::ListItem& item = g_listItems[i];
      item = fui::ListItem{};
      item.label = asText(cell(ctx, top + i, 0));
      item.subtitle = asText(cell(ctx, top + i, 1));
      item.value = asText(cell(ctx, top + i, 2));
      item.actionValue = static_cast<int16_t>(top + i);
      anySubtitle = anySubtitle || item.subtitle != nullptr;
    }
    if (!anySubtitle || rowHeight == subtitledRow) break;
    rowHeight = subtitledRow;
  }

  // The component only ever sees the rows it is about to draw, so it cannot
  // know the list overflows — the shim reserves the track's width and draws the
  // indicator itself, against the real totals.
  fui::Rect rowBand = band;
  const bool overflows = rows > visible;
  if (overflows && tokens.listScrollWidth > 0) {
    const int16_t reserved = static_cast<int16_t>(tokens.listScrollWidth + tokens.listScrollInset + 2);
    const int16_t cut = static_cast<int16_t>(reserved - tokens.listInset);
    if (cut > 0) {
      rowBand.width = static_cast<int16_t>(rowBand.width - cut);
      if (tokens.listScrollSide == 1) rowBand.x = static_cast<int16_t>(rowBand.x + cut);
    }
    fui::drawListScrollIndicator(canvas.target(), band, static_cast<uint32_t>(rows), static_cast<uint32_t>(visible),
                                 static_cast<uint32_t>(top), tokens.listScrollWidth, tokens.listScrollSide,
                                 tokens.listScrollInset);
  }

  fui::ListProps props;
  props.items = g_listItems;
  props.count = static_cast<uint16_t>(shown);
  props.topIndex = 0;
  props.selectedIndex = (selected >= top && selected < top + shown) ? static_cast<int16_t>(selected - top) : -1;
  // xpui declared each row's touch region before rendering and routes taps
  // itself, so the component draws only and registers no competing hits.
  props.action = fui::NO_ACTION;
  props.rowHeight = rowHeight;
  props.valueInset = tokens.spaceMd;  // air between a row's value and its edge
  props.scrollIndicator = false;
  // labelText is deliberately left unset so Screen::list substitutes the
  // theme's body style. Raising its maxLines would let a long label grow its
  // row past ListRowHeight, and xpui counts rows with that metric.

  const fui::DeviceContext device = canvas.device();
  const fui::InputSnapshot noInput{};
  fui::InteractionBuffer<1> hits;
  fui::Frame<1> frame(canvas.target(), device, noInput, hits);

  // Screen::list substitutes the theme's row gap, padding, row styles and
  // selection style. Going through it rather than ui::list keeps that recipe in
  // the SDK, where xpui_fui_metric also reads it from.
  fui::Screen<1> screen(frame, tokens);
  const fui::Rect safe = frame.safeRect();
  screen.setContentMargin(
      fui::Insets{static_cast<int16_t>(rowBand.y - safe.y), static_cast<int16_t>(safe.right() - rowBand.right()),
                  static_cast<int16_t>(safe.bottom() - rowBand.bottom()), static_cast<int16_t>(rowBand.x - safe.x)});
  screen.list(props);
}

void xpui_fui_draw_option_popup(const uint8_t* title, const int32_t count, const int32_t selected,
                                const xpui_fui_cell_fn cell, void* ctx) {
  if (!attached() || !cell || count <= 0) return;

  Canvas canvas;
  // Laid out before the labels are read, because the panel's size depends only
  // on how many options there are; the entries are filled in below and painted
  // from the same array.
  fui::DialogOption entries[MAX_DIALOG_OPTIONS];
  PopupLayout layout;
  if (!popupLayout(canvas.target(), title, count, entries, layout)) return;

  // Only field 0 is meaningful for a dialog's options. The labels stay in
  // xpui's buffers for the call, so the entries borrow them.
  for (int32_t i = 0; i < layout.shown; ++i) {
    const uint8_t* label = cell(ctx, i, 0);
    entries[i].label = label ? asText(label) : "";
    // NO_ACTION throughout: xpui hit-tests with option_popup_row_rect, so the
    // component must not register rects of its own.
    entries[i].action = fui::NO_ACTION;
    entries[i].value = static_cast<int16_t>(i);
    entries[i].state = i == selected ? fui::StateFocused : fui::StateNormal;
  }

  const fui::DeviceContext device = canvas.device();
  const fui::InputSnapshot noInput{};
  fui::InteractionBuffer<1> hits;
  fui::Frame<1> frame(canvas.target(), device, noInput, hits);

  fui::optionDialog(frame, canvas.place(layout.panel), layout.props);
}

uint8_t xpui_fui_option_popup_row_rect(const uint8_t* title, const int32_t count, const int32_t index,
                                       int32_t* out_xywh) {
  if (!attached() || !out_xywh) return 0;

  // Rebuilt from the same layout the painter uses, not read back from a hit
  // buffer: this is asked before the dialog has ever been drawn.
  const fui::DisplayTarget probe(nullptr, 0, 0, 0);  // layout needs font metrics only
  const fui::DialogOption unlabelled[MAX_DIALOG_OPTIONS]{};
  PopupLayout layout;
  if (!popupLayout(probe, title, count, unlabelled, layout)) return 0;

  fui::Rect row;
  if (!popupRowRect(layout, index, row)) return 0;
  out_xywh[0] = row.x;
  out_xywh[1] = row.y;
  out_xywh[2] = row.width;
  out_xywh[3] = row.height;
  return 1;
}

// -- display -----------------------------------------------------------------

// The panel. This shim owns a framebuffer, not a display driver, so the default
// does nothing; a firmware defines this symbol to push the frame it just drew
// (freeink::ui::present(display, hint) is the SDK's one-liner for that).
__attribute__((weak)) void xpui_fui_present(void) {}

void xpui_fui_request_update(void) { xpui_fui_present(); }

}  // extern "C"
