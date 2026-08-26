# SHER Display

The native display server, compositor, and window-management subsystem for
[SHER Kernel](https://github.com/Mullassery/SHER-KERNEL),
[SHER Graphics](https://github.com/Mullassery/SHER-Graphics), and
[SHER Input](https://github.com/Mullassery/SHER-INPUT) — the layer that turns
rendered application surfaces and normalized device activity into an actual,
interactive desktop.

> Compatibility at the boundary, freedom underneath.

The same philosophy [Aurora](https://github.com/Mullassery/aurora) applies to
the desktop and SHER Graphics applies to the GPU, applied here to the
compositor: Wayland and X11 (via XWayland) applications are supported through
compatibility layers at the edge. Underneath them, SHER Display defines its
own native surface, window, and compositor model — this is not a
reimplementation of `wl_display` wearing a different name.

## Why this exists

Every general-purpose Linux compositor inherits thirty years of Wayland's
API-versioning baggage as its *native* interface, then bolts application
isolation, AI-agent control, and headless operation on top as afterthoughts.
SHER Display inverts that: the native model is designed for what a modern
desktop actually needs — structured window operations an AI agent can call
directly instead of simulating mouse and keyboard, security boundaries
enforced by construction instead of by convention, a headless mode that isn't
a hack — and Wayland/X11 compatibility is the thing bolted on at the edge,
where it belongs.

## What's actually here, not just planned

This is an early scaffold, and the numbers below are exact, not rounded up:
14 crates, 56 tests, zero failures, verified before every commit.

A few things worth looking at directly rather than taking on faith:

- **A real cross-repo integration, exercised end to end, not just declared
  as a dependency.** `sher_display_input` drives an actual
  `sher_input_core::InputService` — SHER-Input's real orchestrator — through
  the hermetic `SimulatedController` SHER-Input itself ships for exactly
  this purpose, and asserts on the routed output. No SHER-Display-authored
  mock stands in for the sibling project.
- **Isolation enforced by the type system, not a permission check that can
  be forgotten.** `sher_display_input::RoutedEvent` has no variant that can
  carry an arbitrary surface id — only the tracked focus target. An
  unfocused application has no code path to another application's
  keystrokes; see `input/src/lib.rs`.
- **State machines that reject illegal transitions instead of coercing
  them.** `sher_display_session` will not let you unlock a session that
  isn't locked, or log in twice — see `session/src/lib.rs`.
- **Security that fails closed.** `sher_display_security`'s permission
  grants are time-bound and read as denied the instant they expire, without
  needing an explicit revoke; `sher_display_diagnostics`'s debug-mode gate
  defaults to off and has to be turned on to return anything.
- **Real cross-repo boundary violations, caught and fixed in the open, not
  quietly rewritten out of history.** `sher_display_outputs` originally
  instantiated its own `GPUDriver`, duplicating SHER-Graphics's existing
  ownership of that hardware state — two unsynchronized views of the same
  display. Documented in `VISION.md` under "The SHER-Graphics `GPUDriver`
  ownership decision," because the fix matters more as a precedent than as
  a diff. The same document also records where a *matching* seam
  (SHER-Graphics's hardware-cursor primitives) was confirmed real but
  deliberately left unwired, because wiring it now would mean backing into
  a bigger architectural decision as a side effect.

## The vision and the plan

- [`VISION.md`](./VISION.md) — subsystem ownership boundaries (who owns
  what, and just as importantly, who must not), the display model, protocol
  and backend philosophy, and what "done" actually means.
- [`ROADMAP.md`](./ROADMAP.md) — the full phased plan, with an honest status
  marker on every line: built and tested, partially built, or not started.
  No item claims more than the code backs up.

## Workspace

```
compositor/               frame pipeline: damage tracking, adaptive per-output scheduling
scene/                    geometry primitives + z-ordered scene graph
surfaces/                 application surface state: buffer attach, damage, commit lifecycle
windows/                  window state: layout modes, activation, snap, modal/transient
workspaces/               virtual workspaces: static + dynamic, independent per-output switching
outputs/                  multi-monitor: mirrors SHER-Graphics's connector/mode facts into display policy
input/                    focus-aware routing, global shortcuts, isolation by construction
cursor/                   hardware/software cursor negotiation, theming, accessibility sizing
security/                 time-bound permission grants for capture, recording, injection, clipboard
session/                  explicit session state machine: login, lock, restart, multi-seat
diagnostics/              frame-time/input-latency telemetry, fail-closed debug-mode gate
configuration/            one serializable, machine-readable settings model
compatibility/wayland/    Wayland client lifecycle -> Compositor/WindowManager, no orphaned surfaces
compatibility/xwayland/   X11 window <-> surface-id mapping, deliberately thin
```

Not yet scaffolded (see `ROADMAP.md`): clipboard, drag-and-drop, screenshot,
recording, animation, headless mode, AI/agent APIs.

A structural migration to a leaner `crates/` layout — fewer, larger crates,
plus a native `sher_display_protocol` and a `sher_display_backend`/
`sher_display_linux` split to isolate Ubuntu-specific code — is an open
decision tracked in `ROADMAP.md` Phase 0, not yet executed.

## Cross-repo compatibility (verified, whole family)

This repo is the most cross-repo-coupled member of a 5-repo family under
the Mullassery org, expected to be cloned as sibling directories:
`SHER-Kernel`, `SHER-Graphics`, `SHER-Display` (this repo), `SHER-Input`,
and `Aurora` (GitHub: `SHER-Aurora`). Actual Cargo-level coupling,
confirmed by reading every `Cargo.toml` in the family:

- **SHER-Kernel** — this repo depends on it for `sher_common`,
  `sher_objectmodel`, `gpu_driver`, `wayland_server`, via relative path.
- **SHER-Graphics** — this repo depends on it for `graphics_api`,
  `gpu_abstraction`, `graphics_runtime`, `graphics_compat`.
- **SHER-Input** — this repo depends on it for `sher_input_core`,
  `sher_input_test`.
- **Aurora** — zero coupling; not part of this repo's build.

All three deps are relative paths (`../SHER-Kernel/...`,
`../SHER-Graphics/...`, `../SHER-Input/...`), so this repo's build only
resolves when all four are sibling directories. Verified current: a
from-scratch `cargo build --workspace` compiles clean across all 14 crates
listed above against the current state of all three dependency repos;
`cargo test --workspace` passes 56/56, genuinely exercising the boundary —
`input/` drives SHER-Input's real `InputService` (not a local
reimplementation), and `outputs/` deliberately does **not** instantiate
`gpu_driver::GPUDriver` from SHER-Kernel, only consuming its `Connector`/
`DisplayMode` value types, since SHER-Graphics's `graphics_runtime` is the
crate that actually owns and drives the GPU driver. This ownership split
is the concrete form of this repo's "never instantiate a driver another
subsystem already owns" boundary discipline, and it holds as of this
check — grepped for any other `GPUDriver`/driver-owning construction in
this repo's source and found none.

## Known gaps (external critique, verified)

Two items from the same cross-repo critique pass that fixed real gaps in
`SHER-Kernel` (lock-free/zero-copy IPC ring buffer) and `SHER-Graphics`
(driver panic containment) were also flagged against this repo. Verified
against the actual code rather than assumed:

- **"Formalize IPC/message passing ... for framebuffers/input events" —
  doesn't apply to this repo as stated, by design.** `sher_core::ipc::IpcBus`
  (SHER-Kernel) is now a real lock-free, bounded, zero-copy (`Arc<[u8]>`
  payload) ring buffer — but this repo doesn't depend on `sher_core` and
  doesn't call it anywhere (grepped the whole tree; zero matches). That's
  not an oversight: per this repo's boundary discipline (see "Cross-repo
  compatibility" above), `outputs/` deliberately does not own
  `gpu_driver::GPUDriver` or any framebuffer/pixel-buffer state —
  `SHER-Graphics`'s `graphics_runtime::PresentationBridge` is the sole
  owner of that. There is no framebuffer transport into this repo to
  formalize, because this repo was never supposed to receive raw
  framebuffers in the first place. Input events already flow through a
  different, real mechanism: `input/` subscribes to
  `sher_input_core::InputService`'s `tokio::sync::broadcast` channel
  (SHER-Input's own, not a local reimplementation) — a genuine, audited,
  in-process broadcast primitive, not a naive unbounded queue. Grepped
  `compositor/`, `surfaces/`, and `scene/` for any internal channel/
  `VecDeque`/`Vec<u8>`-based message passing of its own: none exists, so
  there's no framebuffer- or event-scale in-repo data-passing path that's
  currently naive and would benefit from the ring-buffer fix applied
  upstream. Net: nothing to change here without violating the ownership
  split this repo is built around.
- **"Bridge UI toolkit to display server ... Aurora widgets compile down
  to direct scene-graph nodes" — real, intended, not built yet.**
  `SHER-Display` and `SHER-Aurora` are meant to work hand in hand (Aurora as
  the primary shell on top of this compositor — see `ROADMAP.md` Phase 5),
  as is true of the whole family from `SHER-Kernel` through `SHER-Aurora` —
  one interdependent stack at different stages of readiness, not five
  unrelated repos. Today there's genuinely zero Cargo-level coupling
  between this repo and Aurora (no dependency, no shared types, no
  scene-graph API surface exposed by this repo for anything to compile
  down to) — that's a *sequencing* fact (this phase hasn't started; Aurora
  currently renders through literal `gtk4::Button`/`gtk4::Entry` widgets),
  not evidence the integration was abandoned or was never planned. Building
  it means first designing this repo's scene-graph node API as a public
  surface (nothing in `scene/` is exposed for an external toolkit to target
  yet), then Aurora's rendering backend targeting it instead of GTK4 — real
  two-repo work, correctly tracked as not-yet-started in `ROADMAP.md` Phase
  5 rather than fabricated here.

## Building

Prerequisites: Rust 1.75+, with
[`SHER-Kernel`](https://github.com/Mullassery/SHER-KERNEL),
[`SHER-Graphics`](https://github.com/Mullassery/SHER-Graphics), and
[`SHER-Input`](https://github.com/Mullassery/SHER-INPUT) checked out as
**sibling directories** (`../SHER-Kernel`, `../SHER-Graphics`, `../SHER-Input`
relative to this repo) — `sher_common`, `gpu_driver`, `wayland_server`,
`sher_input_core`, and the SHER-Graphics crates are consumed via relative
path dependencies, not published crates yet.

```bash
cargo build --workspace
cargo test --workspace
```

## Known limitations

- Not yet scaffolded: clipboard, drag-and-drop, screenshot, recording,
  animation, headless mode, AI/agent APIs (see the crate list above and
  `ROADMAP.md`). `clipboard/Cargo.toml` exists as a manifest-only stub —
  its `src/` directory exists but is empty (no `.rs` files) — and it is
  not in the workspace `members` list yet.
- The `crates/` layout migration and the `sher_display_protocol` /
  `sher_display_backend` / `sher_display_linux` split are an open decision
  in `ROADMAP.md` Phase 0, not yet executed.
- This repo is not published as crates; it depends on `SHER-Kernel`,
  `SHER-Graphics`, and `SHER-Input` via relative path dependencies, so it
  only builds if those repos are checked out as sibling directories — there
  is no standalone build today.
- No open GitHub issues and no `TODO`/`FIXME` markers in the source as of
  this pass.

## License

Free to use with explicit attribution — see [`LICENSE`](./LICENSE).
