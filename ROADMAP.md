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
  | `compatibility/wayland` (planned) | `crates/sher_display_wayland_compat/` | rename on creation |
  | *(new)* | `crates/sher_display_test/` | cross-crate integration tests |
  | *(new)* | `tools/sher-display-monitor/` | diagnostics CLI (spec v2 section 41) |

- [ ] Define the SHER-Input contract: a trait or small protocol crate
      describing normalized device events, so `sher_display_input` (built
      as a temporary bridge over SHER-Kernel's `input_driver` today) can be
      swapped to consume real SHER-Input later without changing routing
      logic in the compositor.
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
- [ ] Buffer ownership/synchronization as a first-class concept. Today
      `SurfaceState.buffer_id` is just an `ObjectId` handle — no release
      notification, no fence, no "still being written" guard. Needed before
      Phase 3 can safely wire in GPU composition.
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

## Phase 3 — SHER-Graphics Integration

- [ ] Wire `compositor::FrameReport` into `graphics_runtime` for actual GPU
      composition.
- [ ] Buffer sharing / zero-copy path from application buffer through to
      GPU composition.
- [ ] Presentation + frame-timing feedback (vblank, completion fences) back
      into `compositor`'s scheduler.

Not started. Blocked on the buffer-synchronization gap noted in Phase 1.

## Phase 4 — SHER-Input Integration

- [x] `input`: focus-aware key routing (global shortcuts intercept before
      app delivery), pointer-over tracking, keyboard layout switching,
      isolation enforced by construction (an unfocused surface has no code
      path to receive another surface's key events). Currently bridges
      directly to SHER-Kernel's `input_driver` as a stand-in for SHER-Input.
- [ ] Swap the bridge to real SHER-Input once that sibling project exists,
      against the contract defined in Phase 0.
- [ ] Explicit, revocable pointer capture (resize/drag/pointer-lock).
- [ ] Scene-graph hit-testing for pointer targeting — `dispatch_motion`
      currently trusts an externally supplied `pointer_over`; it does not
      yet walk the scene graph itself.
- [ ] Touch, tablet, and gamepad routing.

## Phase 5 — Aurora Integration

- [ ] Privileged desktop-shell client concept (Aurora gets capabilities an
      ordinary application doesn't: global overlays, panel layer-shell-style
      placement).
- [ ] Aurora boots as the primary shell on top of SHER-Display on Ubuntu.

Not started — depends on Phases 1-4 producing a working protocol and
compositor loop.

## Phase 6 — Advanced Display

- [x] `cursor`: hardware/software fallback negotiation, theme, accessibility
      minimum size floor, custom (app-supplied) cursor surfaces.
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
      gated through `security`'s permission model.

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
