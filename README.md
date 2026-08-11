# SHER Display

Native display server, compositor, and window-management subsystem for
[SHER Kernel](https://github.com/Mullassery/SHER-KERNEL) and
[SHER Graphics](https://github.com/Mullassery/SHER-Graphics), applying the
same philosophy [Aurora](https://github.com/Mullassery/aurora) applies to the
desktop layer and SHER Graphics applies to the GPU:

> Compatibility at the boundary, freedom underneath.

Wayland and X11 (via XWayland) applications are supported through
compatibility layers. Underneath them, SHER Display is free to define its own
native surface, window, and compositor architecture — not a reimplementation
of `wl_display`.

See [`VISION.md`](./VISION.md) for the full product vision — subsystem
ownership boundaries, the display model, protocol/backend philosophy, and
what "done" means — and [`ROADMAP.md`](./ROADMAP.md) for the phased plan and
exactly what's built versus planned right now.

## Status

Early scaffold. Nine crates compile and pass 26 tests; no protocol, backend,
or GPU wiring yet. The compositor's frame pipeline runs end to end down to a
`FrameReport` (damage-driven, adaptive per-output scheduling) — presenting
that report through SHER-Graphics is the next real milestone, not yet done.

## Workspace

```
compositor/    # Frame pipeline: damage tracking, adaptive per-output scheduling
scene/         # Geometry primitives + z-ordered scene graph (shared by everything below)
surfaces/      # Application surface state: buffer attach, damage, commit lifecycle
windows/       # Window state: layout modes, activation, snap, modal/transient
workspaces/    # Virtual workspaces: static + dynamic, independent per-output switching
outputs/       # Multi-monitor: mirrors SHER-Graphics's connector/mode facts into display policy
input/         # Focus-aware routing, global shortcuts, isolation-by-construction
cursor/        # Hardware/software cursor negotiation, theming, accessibility sizing
security/      # Time-bound permission grants for capture, recording, injection, clipboard, ...
```

Not yet scaffolded (see `ROADMAP.md`): clipboard, drag-and-drop, screenshot,
recording, animation, session, Wayland/XWayland compatibility layers,
headless mode, AI/agent APIs, diagnostics, configuration.

A structural migration to a leaner `crates/` layout — fewer, larger crates,
plus a native `sher_display_protocol` and a `sher_display_backend`/
`sher_display_linux` split to isolate Ubuntu-specific code — is an open
decision tracked in `ROADMAP.md` Phase 0, not yet executed.

## A note on boundaries

SHER-Kernel owns hardware and low-level transport primitives. SHER-Graphics
owns GPU execution — including the *one* `GPUDriver` instance used for
connector/framebuffer/page-flip state, via its `graphics_runtime` crate.
SHER-Display owns what gets composed and where, not how it's rendered. See
`VISION.md`'s ownership table and its two resolved-boundary-question
write-ups (the SHER-Kernel `wayland_server` deprecation, and why `outputs`
mirrors connector facts instead of owning a second `GPUDriver`) before adding
a new dependency on a SHER-Kernel or SHER-Graphics driver type.

## Prerequisites

- Rust 1.75+
- [`SHER-Kernel`](https://github.com/Mullassery/SHER-KERNEL) and
  [`SHER-Graphics`](https://github.com/Mullassery/SHER-Graphics) checked out
  as **sibling directories** (`../SHER-Kernel`, `../SHER-Graphics` relative to
  this repo) — `sher_common`, `sher_objectmodel`, `gpu_driver`,
  `input_driver`, `wayland_server`, and the SHER-Graphics crates are consumed
  via relative path dependencies, not published crates yet.

## Building

```bash
cargo build --workspace
cargo test --workspace
```

## License

Free to use with explicit attribution — see [`LICENSE`](./LICENSE).
