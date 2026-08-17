// The lifecycle a host drives a Rust screen through.
//
// The other half of the boundary. `xpui_fui.h` is what Rust calls to draw;
// this is what the host calls to run a screen at all — and the direction is
// reversed, so these are DEFINED IN RUST, in `src/lifecycle.rs`, and this
// header is only the declaration. There is no `.cpp` beside it to compile.
//
// A screen is an opaque handle. It comes from a factory the application
// exports (see the `register_screen!` macro) and is passed back to every call
// below until `xpui_screen_destroy` frees it. Nothing on this side may look
// inside it, and a handle used after destroy is a use-after-free rather than a
// null-pointer crash — every function here tolerates NULL and nothing else.
//
// Ordering, as a host must call them:
//
//   handle = create();          the application's factory
//   xpui_screen_on_enter(handle);
//   repeat {
//     xpui_screen_loop(handle);     input, once per frame
//     xpui_screen_render(handle);   paint, only when a frame is wanted
//   }
//   xpui_screen_on_exit(handle);
//   xpui_screen_destroy(handle);
//
// `loop` before `render` matters: a screen asks for its repaint from inside
// `loop`, through `xpui_fui_request_update`, so a host that renders first
// paints the frame before the one it was asked for.

#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// The screen is being shown. Call once, before the first loop or render.
void xpui_screen_on_enter(void* screen);

// One frame of input. Call once per host frame, whether or not anything
// arrived: a screen with a deadline has nowhere else to notice it passed.
void xpui_screen_loop(void* screen);

// The screen is coming off the stack. The handle is still valid afterwards —
// destroying it is a separate call, so a host may keep it to resume.
void xpui_screen_on_exit(void* screen);

// Paints into the framebuffer given to `xpui_fui_attach`. Takes no renderer:
// what paints is whatever was installed on the Rust side, and a parameter
// nothing reads is one that can silently go out of step with this header.
void xpui_screen_render(void* screen);

// Offers the system home gesture. Returns non-zero when the screen consumed
// it — an overlay does, so the gesture dismisses the overlay rather than the
// screen underneath. A host that gets 0 back applies its own meaning, which is
// usually "return to the root screen".
uint8_t xpui_screen_home_gesture(void* screen);

// Drops the screen. The handle is dangling afterwards.
void xpui_screen_destroy(void* screen);

#ifdef __cplusplus
}  // extern "C"
#endif
