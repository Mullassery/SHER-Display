# SHER Display

The native display server, compositor, and window-management subsystem for the SHER OS family.

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE)
[![CI](https://github.com/Mullassery/SHER-Display/actions/workflows/ci.yml/badge.svg)](https://github.com/Mullassery/SHER-Display/actions/workflows/ci.yml)

## What it is

SHER Display is the layer that turns rendered application surfaces and normalized device activity — from [SHER-Graphics](https://github.com/Mullassery/SHER-Graphics) and [SHER-INPUT](https://github.com/Mullassery/SHER-INPUT) — into an actual, interactive desktop: a compositor, a scene graph, window and workspace management, focus routing, cursor handling, session state, and a capability-based security boundary.

It is a 14-crate Rust workspace (about 56 tests, zero failures) built around a native surface/window/compositor model rather than a renamed `wl_display`. Wayland (and X11 via XWayland) compatibility is layered at the edge, where it belongs; underneath, the native model is designed for what a modern desktop needs — structured window operations an AI agent can call directly instead of simulating mouse and keyboard.

## Features / Highlights

- **Native compositor pipeline** — damage tracking and adaptive per-output scheduling (`compositor`), a z-ordered scene graph (`scene`), application surface lifecycle (`surfaces`), window state/layout/snap (`windows`), and virtual workspaces (`workspaces`).
- **Multi-output display policy** — mirrors SHER-Graphics's connector/mode facts without re-owning the GPU (`outputs`).
- **Input routing with isolation by construction** — `sher_display_input` drives a real `sher_input_core::InputService` (no local mock), and `RoutedEvent` has no variant that can carry an arbitrary surface id, so an unfocused application has no code path to another application's keystrokes.
- **Focus, shortcuts, cursor, and security** — focus-aware routing and global shortcuts (`input`), hardware/software cursor negotiation (`cursor`), and time-bound permission grants for capture/recording/injection/clipboard (`security`).
- **Explicit session state machine** — login, lock, restart, and multi-seat transitions that reject illegal state changes instead of coercing them (`session`).
- **Fail-closed diagnostics** — frame-time and input-latency telemetry behind a debug gate that defaults to off (`diagnostics`).
- **Wayland and XWayland compatibility seams** — Wayland client lifecycle mapped to the native compositor/window manager with no orphaned surfaces (`compatibility/wayland`); a deliberately thin X11 window ↔ surface-id mapping (`compatibility/xwayland`).
- **One serializable settings model** (`configuration`).

## Who it's for / Use cases

- **Prototyping a compositor and window-management model where AI-agent control is a first-class primitive**, rather than simulated keyboard/mouse input bolted onto an existing display server.
- **Studying cross-repo boundary discipline** in a multi-crate Rust workspace — the `GPUDriver` ownership fix and the documented "confirmed real but deliberately unwired" seam in `VISION.md` are worked examples.
- **Testing an input → compositor → window-routing pipeline** without real hardware, via SHER-INPUT's shipped `SimulatedController`.

## What it is not

- **Not yet a usable desktop.** Clipboard, drag-and-drop, screenshot, recording, animation, headless mode, and AI/agent APIs are not scaffolded. `clipboard/Cargo.toml` exists as a manifest-only stub with no `src/`, and is not yet in the workspace `members` list; the other listed features have no directory or manifest at all.
- **Not a standalone build.** It depends on `SHER-KERNEL`, `SHER-Graphics`, and `SHER-INPUT` via relative path dependencies, so all four must be checked out as sibling directories.
- **Not a reimplementation of Wayland as its native model.** Wayland/X11 are compatibility layers at the edge; the native surface/window/compositor model is its own design.
- **Not its own GPU owner.** `outputs/` consumes SHER-Graphics's value types only and never instantiates `gpu_driver::GPUDriver` — SHER-Graphics's `graphics_runtime` owns and drives the GPU.
- **Not published as crates.** No registry packages; path dependencies only.
- **Not a finished security boundary.** Permission grants are time-bound and fail closed, but this is pre-alpha scaffolding, not a hardened, audited display server.

## Project status

- **Implemented and tested (14 crates, 56 tests):** `compositor`, `scene`, `surfaces`, `windows`, `workspaces`, `outputs`, `input`, `cursor`, `security`, `compatibility/wayland`, `compatibility/xwayland`, `session`, `diagnostics`, `configuration`.
- **Not yet scaffolded:** clipboard (manifest-only stub), drag-and-drop, screenshot, recording, animation, headless mode, AI/agent APIs (see [`ROADMAP.md`](./ROADMAP.md)).
- **Open decision, not executed:** a migration to a leaner `crates/` layout plus a native `sher_display_protocol` and a `sher_display_backend`/`sher_display_linux` split (tracked in `ROADMAP.md` Phase 0).
- **No tagged releases yet** — everything is unreleased, pre-alpha (see [`CHANGELOG.md`](./CHANGELOG.md)).

## Installation

Requires Rust 1.75+, with [`SHER-KERNEL`](https://github.com/Mullassery/SHER-KERNEL), [`SHER-Graphics`](https://github.com/Mullassery/SHER-Graphics), and [`SHER-INPUT`](https://github.com/Mullassery/SHER-INPUT) checked out as **sibling directories** (`../SHER-KERNEL`, `../SHER-Graphics`, `../SHER-INPUT` relative to this repo), because those crates are consumed via relative path dependencies, not published packages:

```bash
git clone https://github.com/Mullassery/SHER-KERNEL.git
git clone https://github.com/Mullassery/SHER-Graphics.git
git clone https://github.com/Mullassery/SHER-INPUT.git
git clone https://github.com/Mullassery/SHER-Display.git
cd SHER-Display
cargo build --workspace
```

## Quick start

```bash
cargo build --workspace
cargo test --workspace
```

## Usage

This is a library workspace with no runnable binary yet; it is exercised through its crates and tests. The most direct entry points are `sher_display_input` (focus-aware routing over a real SHER-INPUT pipeline), `sher_display_session` (session state machine), and `sher_display_security` (time-bound permission grants). See [`docs/architecture/README.md`](./docs/architecture/README.md) for the crate dependency graph.

## Documentation

- [`VISION.md`](./VISION.md) — subsystem ownership boundaries, the display model, and protocol/backend philosophy
- [`ROADMAP.md`](./ROADMAP.md) — the full phased plan with an honest status marker on every line
- [`ROADMAP_HONEST.md`](./ROADMAP_HONEST.md) — technical-debt inventory and CI/tooling gaps
- [`docs/architecture/README.md`](./docs/architecture/README.md) — Mermaid diagrams of cross-repo layering and internal crate dependencies
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — setup and the pre-PR checklist
- [`SECURITY.md`](./SECURITY.md) — how to report a security issue privately
- [`CHANGELOG.md`](./CHANGELOG.md) — change history

## Contributing

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for setup and the pre-PR checklist, [`SECURITY.md`](./SECURITY.md) to report a security issue privately, and [`CODE_OF_CONDUCT.md`](./CODE_OF_CONDUCT.md). Change history is in [`CHANGELOG.md`](./CHANGELOG.md) (no tagged releases yet — everything is unreleased, pre-alpha).

## Related projects

- [SHER-KERNEL](https://github.com/Mullassery/SHER-KERNEL) — AI-native operating-system kernel with zero-trust security
- [SHER-Display](https://github.com/Mullassery/SHER-Display) — display server, compositor, and window manager for the SHER OS family
- [SHER-Graphics](https://github.com/Mullassery/SHER-Graphics) — native graphics architecture (software GPU + Vulkan backend)
- [SHER-INPUT](https://github.com/Mullassery/SHER-INPUT) — input subsystem that normalizes device activity into a canonical event stream
- [SHER-Aurora](https://github.com/Mullassery/aurora) — GNOME design system for Rust
- [SHER-Process-Explorer](https://github.com/Mullassery/SHER-Process-Explorer) — evidence-based Linux process explorer
- [Browse all projects](https://github.com/Mullassery?tab=repositories)

## License

This project is licensed under the [Apache License 2.0](./LICENSE).
