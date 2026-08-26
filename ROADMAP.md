# SHER-Display: Roadmap

Status reflects the actual repo on disk, not intent. `[x]` compiles and has
passing tests; `[~]` partially built or a manifest-only stub; `[ ]` not
started.

## Phase 0 — Foundation & Ownership Cleanup

- [x] Deprecate SHER-Kernel's `WaylandCompositor`; extract `WaylandTransport`
      (client connections + buffer handles) as the low-level primitive
      SHER-Display consumes. 18 tests passing in `SHER-Kernel/crates/wayland_server`.
- [x] Stand up the SHER-Display Cargo workspace, path dependencies on
      SHER-Kernel (`sher_common`, `sher_objectmodel`, `gpu_driver`,
      `input_driver`, `wayland_server`) and SHER-Graphics (`graphics_api`,
      `gpu_abstraction`, `graphics_runtime`, `graphics_compat`).
- [ ] **Decision pending — structural migration.** Two specs produced two
      different crate layouts: a flat, fine-grained one (~20 top-level
      crates) and a consolidated one under `crates/` (fewer, larger crates,
      explicitly rejecting "artificial modularity"). The 9 crates built so
      far are small and cohesive enough to consolidate cheaply. Target
      layout, pending confirmation to execute the move:

  | Existing (flat) | Target (`crates/`) | Action |
  |---|---|---|
  | `scene/` | `crates/sher_display_core/` | rename; also absorbs `session`, `configuration` once written |
  | `surfaces/` | `crates/sher_display_surface/` | rename |
  | `windows/`, `workspaces/` | `crates/sher_display_window/` | merge |
  | `outputs/` | `crates/sher_display_output/` | rename |
  | `compositor/`, `cursor/` | `crates/sher_display_compositor/` | merge (cursor is a composited object, not a standalone concern) |
  | `input/` | stays inside `sher_display_compositor` or a thin `sher_display_input_bridge` | pending SHER-Input contract design below |
  | `security/` | `crates/sher_display_core/` | merge (permissions are core, not a feature crate) |
  | *(new)* | `crates/sher_display_protocol/` | native client/compositor protocol — not started |
  | *(new)* | `crates/sher_display_backend/` | backend trait — not started |
  | *(new)* | `crates/sher_display_linux/` | Ubuntu/DRM/evdev backend impl — not started |
  | `compatibility/wayland` (built as `sher_display_compat_wayland`) | `crates/sher_display_wayland_compat/` | move + rename package |
  | *(new)* | `crates/sher_display_test/` | cross-crate integration tests |
  | *(new)* | `tools/sher-display-monitor/` | diagnostics CLI (spec v2 section 41) |

- [x] SHER-Input contract: turned out to not be SHER-Display's to define —
      SHER-Input (`Mullassery/SHER-INPUT`) now exists as a real, independent
      repo with its own canonical `InputEvent`/`InputEventPayload` model,
      `InputService` orchestrator, and `CaptureRegistry`. `sher_display_input`
      consumes it directly; see Phase 4 and VISION.md's "SHER-Input
      integration" section.
- [x] Boundary audit against SHER-Graphics/SHER-Input: found and fixed a
      real violation — `sher_display_outputs::OutputManager` was
      instantiating its own `gpu_driver::GPUDriver`, duplicating
      `graphics_runtime::PresentationBridge`'s existing ownership of that
      hardware state. Fixed by making `OutputManager` a pure mirror over
      `Connector`/`DisplayMode` facts (see VISION.md, "The SHER-Graphics
      `GPUDriver` ownership decision"). `compositor`, `cursor`, `input`,
      `surfaces`, `windows`, `workspaces`, `security` audited clean — none
      construct a driver/hardware handle that competes with an existing
      owner in SHER-Kernel or SHER-Graphics.

## Phase 1 — Display Foundation

- [x] `scene`: geometry primitives (`Point`/`Size`/`Rect`/`Transform`) and a
      z-ordered scene graph with damage accumulation and per-output
      composition filtering.
- [x] `surfaces`: surface state machine (create → attach buffer → damage →
      commit → frame callback), per-client teardown.
- [x] `compositor`: damage-driven adaptive frame scheduling, independent
      refresh interval per output, produces a `FrameReport` (composited node
      count, damage regions, full-redraw flag) — stops short of GPU
      submission by design (Phase 3).
- [x] `outputs`: mirrors `gpu_driver::Connector`/`DisplayMode` facts (does
      **not** own a `GPUDriver` — see Phase 0's boundary audit) into
      multi-monitor policy: hotplug with guaranteed non-empty output set
      (virtual fallback), independent scale/position/orientation per
      output.
- [~] `clipboard`: manifest created, implementation not started — was
      mid-flight when spec v2 arrived; folding scope into the leaner crate
      set is part of the pending structural migration.
- [x] `compatibility/wayland` (`sher_display_compat_wayland`): translates
      client-connect/create-surface/attach-buffer/damage/commit/destroy
      into calls against `Compositor`/`WindowManager`, guarantees no
      orphaned surfaces on client disconnect. Owns only the kernel-facing
      `WaylandTransport` plus its own protocol bookkeeping (surface→node,
      surface→window, surface→client maps) — never owns `Compositor` or
      `WindowManager` itself, both are taken as `&mut` parameters so
      whatever eventually assembles the session is the one owner.
- [ ] Buffer ownership/synchronization as a first-class concept. Today
      `SurfaceState.buffer_id` is just an `ObjectId` handle — no release
      notification, no fence, no "still being written" guard. Needed before
      Phase 3 can safely wire in GPU composition. (External critique proposed
      this as "adopt a lock-free zero-copy IPC ring buffer for framebuffers" —
      verified there's no naive-copy problem to fix today, since input/kernel
      data already arrives in-process via `Arc<InputService>` +
      `tokio::sync::broadcast`, not syscalls. The real gap is buffer
      release/fence semantics on the handle above, not a new IPC subsystem —
      and building buffer transport in SHER-Display itself would violate this
      repo's own driver-ownership boundary: SHER-Kernel keeps the transport
      primitive, SHER-Display keeps the policy, per VISION.md's resolved
      `WaylandCompositor`/`GPUDriver` ownership decision. Do not implement the
      critique as literally proposed.)
- [ ] `sher_display_protocol`: the native client/compositor contract
      (Connect, CreateSurface, AttachBuffer, CommitSurface, SetGeometry,
      RequestFrame, Configure, Close, ...). Not started.
- [ ] `sher_display_backend` trait + `sher_display_linux` implementation.
      Not started — `outputs`/`compositor` currently call `gpu_driver`
      directly rather than through a backend seam.
- [ ] Display server lifecycle / client connection management. There is
      currently no "client connects, gets a protocol session" concept above
      `WaylandTransport`'s raw `WaylandClient`.

## Phase 2 — Window System

- [x] `windows`: layout modes (floating/tiled/maximized/fullscreen), snap,
      modal/transient windows, always-on-top, activation with
      single-active-window invariant enforced.
- [x] `workspaces`: static + dynamic workspaces, per-output independent
      active workspace, window assignment/movement.
- [ ] Formal window state machine with rejected invalid transitions (spec
      v2 section 8: Created → Mapped → Visible → {Focused, Maximized,
      Fullscreen, Minimized} → Unmapped → Destroyed). `windows` currently
      has the state *fields* but no transition table rejecting illegal
      moves (e.g. nothing stops focusing a destroyed window).
- [ ] Parent/child surface trees for popups, tooltips, dialogs, dropdowns
      (spec v2 section 6). `SurfaceRole::Popup` exists but carries no
      parent-surface link yet.
- [ ] Configure-event negotiation (compositor proposes geometry, client
      acks) rather than the client's request being applied unconditionally.

- [x] `compatibility/xwayland` (`sher_display_compat_xwayland`): X11 XID
      ↔ surface-id mapping table. Deliberately thin — spawning the real
      XWayland process and driving it as an ordinary Wayland client through
      `WaylandBridge` is left to whoever assembles the session; this crate
      only resolves X11-terms window-management requests to a surface id.
- [x] `session`: explicit `LoggedOut → Active → Locked` state machine
      (spec section 33), illegal transitions rejected (e.g. unlock without
      being locked), multi-seat tracking independent of login state.
- [x] `diagnostics`: rolling frame-time/input-latency averages, dropped/
      missed frame counters, the `sher-display-monitor` snapshot shape
      from spec section 35 — and a fail-closed `DebugMode` gate for the
      section 36 requirement that debug tooling stay off by default.
      Doesn't measure GPU/CPU itself (out of its boundary — see
      VISION.md); only aggregates values pushed into it.
- [x] `configuration`: one serializable `DisplayConfig` (spec section 37)
      covering per-output settings, workspace/window/animation/cursor/
      accessibility/touchpad/power behavior, and keyboard shortcuts —
      JSON round-trip via `to_json`/`from_json`.

## Phase 3 — SHER-Graphics Integration

- [ ] Wire `compositor::FrameReport` into `graphics_runtime` for actual GPU
      composition.
- [ ] Buffer sharing / zero-copy path from application buffer through to
      GPU composition.
- [ ] Presentation + frame-timing feedback (vblank, completion fences) back
      into `compositor`'s scheduler.
- [ ] Decide how SHER-Display parameterizes over `GraphicsRuntime<D:
      GpuDriver>`'s driver type generic, so `outputs`, `compositor`, and
      `cursor` (see Phase 6) can all reach it consistently instead of each
      crate making its own ad hoc choice. `graphics_runtime` also gained a
      capability-gated `driver_mut()` escape hatch and command-stream
      validation since the last audit — worth a pass once real wiring
      starts, not blocking now since nothing calls into it yet.

Not started. Blocked on the buffer-synchronization gap noted in Phase 1.

## Phase 4 — SHER-Input Integration

- [x] `input` (`sher_display_input`): rewritten to consume real
      `sher_input_core::InputService` — the temporary bridge to
      SHER-Kernel's `input_driver` is gone entirely, not just supplemented.
      Focus-aware routing (keyboard focus, pointer-over target), global
      shortcuts intercept before app delivery, isolation enforced by
      construction (`RoutedEvent::Focused` can only ever name the tracked
      focus target). Verified against a real `InputService` driven by
      `sher_input_test::SimulatedController` — 5 tests, all passing, no
      SHER-Display-authored mocks.
- [x] Explicit, revocable pointer capture — `InputRouter::request_pointer_capture`
      is a thin pass-through to SHER-Input's `CaptureRegistry`; SHER-Input
      enforces exclusivity/single-owner/revocable, SHER-Display only decides
      *when* to ask for it (drag/resize/pointer-lock policy stays here).
- [x] Keyboard layout switching — pass-through to
      `InputService::set_layout`; SHER-Input owns the mapping mechanism
      (`KeyboardLayout` trait, `UsQwertyLayout` today), SHER-Display owns
      which layout the user configured.
- [ ] Scene-graph hit-testing for pointer targeting — `pointer_over` is
      still externally supplied by whatever calls `set_pointer_over`; the
      router does not yet walk the scene graph itself to compute it.
- [x] Touch, tablet, and gamepad routing — reach `InputRouter` through the
      same canonical `InputEventPayload` the keyboard/pointer cases do
      (`Touch`/`Tablet` route to `pointer_over`, `Gamepad` to
      `keyboard_focus`); not yet exercised by a dedicated test.

## Phase 5 — Aurora Integration

`SHER-Display` and `SHER-Aurora` are intended to work hand in hand — Aurora
is the planned primary shell on top of this compositor. That intent is
correct; what needed fixing was a timing/status mismatch, not the pairing
itself.

**Current state, verified:** `SHER-Aurora` today has **zero Cargo-level
dependency** on any SHER repo and renders through literal
`gtk4::Button`/`gtk4::Entry` widgets with GTK4's native event handling, not
a scene-graph-targeting abstraction — because this phase (and the
`sher_display_protocol`/scene-graph work it needs from `SHER-Display`'s side)
hasn't started, matching "Not started — depends on Phases 1-4" below. Aurora
being GNOME/GTK4-only *today* describes its current, pre-integration state,
not a refusal to integrate — `SHER-Aurora`'s own docs are being updated
alongside this to say so explicitly, so a future reader doesn't mistake
"not wired up yet" for "not going to happen."

- [ ] Privileged desktop-shell client concept (Aurora gets capabilities an
      ordinary application doesn't: global overlays, panel layer-shell-style
      placement).
- [ ] Aurora boots as the primary shell on top of SHER-Display on Ubuntu.

Not started — depends on Phases 1-4 producing a working protocol and
compositor loop, and on `SHER-Display` exposing a scene-graph API for
Aurora's rendering backend to target (nothing in `scene/` is exposed for
that yet either).

## Phase 6 — Advanced Display

- [x] `cursor`: hardware/software fallback negotiation, theme, accessibility
      minimum size floor, custom (app-supplied) cursor surfaces. The real
      call site is confirmed: `GraphicsRuntime::{set_cursor_image,
      set_cursor_position, show_cursor, hide_cursor}` (see VISION.md's
      "SHER-Graphics cursor seam" section) — not wired yet, pending the
      Phase 3 driver-generic decision above.
- [x] `security`: time-bound permission grants for screen capture,
      recording, input injection, clipboard access, window inspection,
      global shortcuts, display configuration, remote display, and
      accessibility privileges — mirrors SHER-Kernel's capability model
      (fail-secure: absent or expired grant reads as denied, never checked
      implicitly against a live clock so expiry is deterministic in tests).
- [ ] Clipboard (streaming/chunked for large payloads, not just small blobs),
      drag-and-drop, screenshot, screen recording. None started; scope needs
      re-checking against the leaner crate set before writing more code.
- [ ] Fractional scaling refinement, output rotation/flip, HDR/color
      management.
- [ ] Headless/virtual-display mode, structured AI/agent APIs
      (`move_window`, `arrange_windows`, permitted screenshot capture)
      gated through `security`'s permission model. (External critique
      assumed this was already real, citing the top-level README's
      "headless mode built in" claim — verified that's aspirational: the
      `headless/src/` crate has no `.rs` files and isn't in the workspace
      `members` list. README's own "Known limitations" section already
      admits this; flagging here since the top-level positioning claim
      should be softened until this phase actually lands.)

## Phase 7 — Native SHER Backend

Remove Ubuntu/Linux assumptions once SHER-Kernel and SHER-Graphics no longer
sit on Ubuntu; implement a native SHER backend behind the same
`sher_display_backend` trait used by `sher_display_linux`. Not started —
explicitly long-term; the backend abstraction in Phase 1 exists precisely so
this phase doesn't require touching the core compositor.

## What "done" looks like

Ubuntu boots into Aurora running on SHER-Display. A user can see the desktop,
launch applications, create/move/resize/maximize/minimize/fullscreen windows,
switch focus, use popups and menus, run multiple displays with independent
scaling, use keyboard/pointer input, close applications, and have the
compositor survive an application crash, a monitor disconnect, or a GPU error
without the rest of the desktop going down. See `VISION.md` for the full
definition of success and the boundaries each subsystem must respect to get
there.
