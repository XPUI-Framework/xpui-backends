// The lifecycle a host drives a Rust screen through.
//
// The other half of the boundary: `xpui_fui.h` is what Rust calls to draw;
// these are what the host calls to run a screen, and they are DEFINED IN
// RUST, in `src/lifecycle.rs` — this header is only the declaration.
//
// A screen is an opaque handle from a factory the application exports (see
// `register_screen!`), passed back to every call until `xpui_screen_destroy`
// frees it. Every function tolerates NULL and nothing else; a handle used
// after destroy is a use-after-free. Call on_enter once, then loop and
// render per frame — `loop` before `render`, because a screen asks for its
// repaint from inside `loop` — then on_exit, then destroy.

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
