# A screen's lifecycle over C

How a C++ host that owns its own screen stack drives an `xpui` screen: it holds
the screen as an opaque handle and calls six `extern "C"` entry points, which
`cpp/xpui_screen.h` declares for C++. They are defined here, in Rust, because
they are the C boundary.

**The handle is boxed twice.** `Driver` is a trait object, so `Box<dyn Driver>`
is a fat pointer and cannot cross as one `void*`: the inner box erases the
screen's type, the outer one gives a thin pointer. Every function here reverses
exactly that, and getting it wrong is a wild pointer rather than a compile
error, so a handle only ever comes from [`into_handle`](#xpui_fuilifecycleinto_handle),
[`handle_for`](#xpui_fuilifecyclehandle_for) or a
[`register_screen!`](backend.md#xpui_fuiregister_screen) factory.

## Topics

| | |
|---|---|
| [`into_handle`](#xpui_fuilifecycleinto_handle) | Wraps a screen for a C++ host to own, returning the opaque handle. |
| [`handle_for`](#xpui_fuilifecyclehandle_for) | Wraps a screen whose type has already been erased, returning the opaque handle. |
| [`reclaim`](#xpui_fuilifecyclereclaim) | Takes a screen back out of a handle the host declined. |
| [`xpui_screen_on_enter`](#xpui_fuilifecyclexpui_screen_on_enter) | The screen is being shown. |
| [`xpui_screen_loop`](#xpui_fuilifecyclexpui_screen_loop) | One frame of input. |
| [`xpui_screen_render`](#xpui_fuilifecyclexpui_screen_render) | Paints the screen into whatever `xpui_fui_attach` was given. |
| [`xpui_screen_home_gesture`](#xpui_fuilifecyclexpui_screen_home_gesture) | Offers the system home gesture, answering non-zero when the screen took it. |
| [`xpui_screen_on_exit`](#xpui_fuilifecyclexpui_screen_on_exit) | The screen is being taken off the stack. |
| [`xpui_screen_destroy`](#xpui_fuilifecyclexpui_screen_destroy) | Drops the screen, leaving the handle dangling. |

**Example — one screen, through its whole life**

```rust
use xpui::screen::Screen;
use xpui::{NavigationScreen, Text, View};
use xpui_fui::lifecycle::{
    into_handle, xpui_screen_destroy, xpui_screen_home_gesture, xpui_screen_loop,
    xpui_screen_on_enter, xpui_screen_on_exit, xpui_screen_render,
};
use xpui_fui::{Backend, NoInput};

static PLATFORM: NoInput = NoInput;
static BACKEND: Backend<NoInput> = Backend::new(&PLATFORM);

struct About;

impl Screen for About {
    type Message = ();

    fn body(&self) -> impl View<()> {
        NavigationScreen::new(Text::new("Version 1.4.2")).title("About")
    }

    fn update(&mut self, _message: ()) {}
}

// Safety: one thread, and nothing has rendered yet.
unsafe { xpui::host::install(&BACKEND) };

let handle = into_handle(About);
// Safety: `handle` is live for every call, and destroyed exactly once.
unsafe {
    xpui_screen_on_enter(handle);
    xpui_screen_loop(handle); // once per host frame
    xpui_screen_render(handle);
    assert_eq!(xpui_screen_home_gesture(handle), 0, "About does not take it");
    xpui_screen_on_exit(handle);
    xpui_screen_destroy(handle);
}
```

## `xpui_fui::lifecycle::into_handle`

Wraps a screen for a C++ host to own, returning the opaque handle.

```text
pub fn into_handle<S: Screen + 'static>(screen: S) -> *mut c_void
```

The screen is live from here on and belongs to the caller, which must
eventually pass the handle to [`xpui_screen_destroy`](#xpui_fuilifecyclexpui_screen_destroy)
or [`reclaim`](#xpui_fuilifecyclereclaim).

## `xpui_fui::lifecycle::handle_for`

Wraps a screen whose type has already been erased, returning the opaque handle.

```text
pub fn handle_for(screen: Box<dyn Driver>) -> *mut c_void
```

What lets a Rust screen push another onto a stack that lives in C++:
`Navigator::present` is handed a `Box<dyn Driver>`, and a navigator with no way
to turn that into a handle could only hand it back.

## `xpui_fui::lifecycle::reclaim`

Takes a screen back out of a handle the host declined.

```text
pub unsafe fn reclaim(handle: *mut c_void) -> Option<Box<dyn Driver>>
```

The mirror of [`handle_for`](#xpui_fuilifecyclehandle_for). A host that
refuses a push has not taken ownership, so the screen is reclaimed and handed
back to whoever tried to present it rather than leaked. `None` for a null
handle.

> [!WARNING]
> **Safety.** `handle` must be null, or a handle from `into_handle` or
> `handle_for` that has not been destroyed or reclaimed already. Reclaiming one
> the host did take frees a screen it is still driving.

```rust
use xpui::screen::Screen;
use xpui::{Text, View};
use xpui_fui::lifecycle::{into_handle, reclaim};

struct Declined;

impl Screen for Declined {
    type Message = ();
    fn body(&self) -> impl View<()> {
        Text::new("never shown")
    }
    fn update(&mut self, _message: ()) {}
}

let handle = into_handle(Declined);
// Safety: a live handle the host refused, reclaimed once.
let screen = unsafe { reclaim(handle) };
assert!(screen.is_some());
// Safety: null is always accepted.
assert!(unsafe { reclaim(core::ptr::null_mut()) }.is_none());
```

## The entry points

Every one of these takes the handle and does nothing for a null one.

> [!WARNING]
> **Safety, for all six.** `handle` must be null or a live handle from
> `into_handle` or `handle_for`, not yet destroyed or reclaimed.

## `xpui_fui::lifecycle::xpui_screen_on_enter`

The screen is being shown.

```text
pub unsafe extern "C" fn xpui_screen_on_enter(handle: *mut c_void)
```

Nothing is installed here: a host installs the backend once, before the first
screen exists.

## `xpui_fui::lifecycle::xpui_screen_loop`

One frame of input.

```text
pub unsafe extern "C" fn xpui_screen_loop(handle: *mut c_void)
```

Call once per host frame, before rendering.

## `xpui_fui::lifecycle::xpui_screen_render`

Paints the screen into whatever `xpui_fui_attach` was given.

```text
pub unsafe extern "C" fn xpui_screen_render(handle: *mut c_void)
```

It takes no renderer: what paints is the installed host.

## `xpui_fui::lifecycle::xpui_screen_home_gesture`

Offers the system home gesture, answering non-zero when the screen took it.

```text
pub unsafe extern "C" fn xpui_screen_home_gesture(handle: *mut c_void) -> u8
```

A host acts on the gesture itself only when this answers `0`.

## `xpui_fui::lifecycle::xpui_screen_on_exit`

The screen is being taken off the stack.

```text
pub unsafe extern "C" fn xpui_screen_on_exit(handle: *mut c_void)
```

## `xpui_fui::lifecycle::xpui_screen_destroy`

Drops the screen, leaving the handle dangling.

```text
pub unsafe extern "C" fn xpui_screen_destroy(handle: *mut c_void)
```

**See also:** [`register_screen!`](backend.md#xpui_fuiregister_screen), [`reclaim`](#xpui_fuilifecyclereclaim)
