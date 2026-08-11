# SHER-Display: Product Vision

## Mission

SHER-Display turns what SHER-Graphics can render into an actual interactive
desktop. It determines what surfaces exist, where they appear, how they are
composed, which output owns them, and how display-level interaction is
routed to them. It is the layer that makes SHER-Kernel and SHER-Graphics
usable as a graphical operating system, and the foundation Aurora builds the
desktop experience on.

It is not "another Wayland compositor." It is SHER's native display
architecture — a first-class SHER protocol at its core, with Wayland/XWayland
compatibility provided as boundary layers rather than as the thing itself.

## Immediate deployment target vs. long-term architecture

Short term, SHER-Display ships on:

```
Ubuntu + SHER-Kernel + SHER-Graphics + SHER-Display + SHER-Input + Aurora
```

Ubuntu-specific and Linux-specific integration (DRM/KMS discovery, evdev,
Wayland/XWayland compatibility) must stay isolated behind a backend
abstraction. The core compositor, surface/window model, and protocol must not
encode Ubuntu or Linux assumptions, so that a native SHER OS backend can
replace the Linux backend later without touching SHER-Display's core.

## Ownership boundaries

Five subsystems, five distinct questions. No subsystem answers another's
question.

| Subsystem | Question it answers | Must NOT own |
|---|---|---|
| **SHER-Kernel** | How does the machine operate? (hardware, memory, scheduling, low-level IPC/transport primitives, device primitives) | Desktop compositor policy |
| **SHER-Graphics** | How does SHER execute rendering on the GPU? (GPU abstraction, rendering contexts, GPU synchronization, Vulkan/OpenGL/Mesa compatibility) | Window focus, desktop policy |
| **SHER-Input** | What physical interaction occurred? (device lifecycle, canonical ordered/sequenced/coalesced event stream, keyboard-layout mapping, low-level capture enforcement) | Which application/window an event belongs to |
| **SHER-Display** *(this repo)* | Where do graphical surfaces, windows, and displays live, and where does interaction go? (surfaces, windows, buffers, outputs, compositor, composition, frame scheduling, damage, focus, coordinate transforms, display protocol, input-event routing) | Rendering execution, desktop visual policy |
| **Aurora** | What does the interaction mean to the desktop UX? (panels, launcher, widgets, settings, visual language, desktop policy, GTK4/libadwaita design system) | Compositor/window-management mechanism |

SHER-Display consumes SHER-Graphics for rendering execution and SHER-Input
for normalized device events. It does not duplicate either. Concretely:
`sher_display_compositor` decides *what* needs recomposing and produces a
`FrameReport`; wiring that into `graphics_runtime` for GPU execution is
SHER-Graphics integration work, not something SHER-Display re-implements.
`sher_display_input` decides *where* an already-normalized event goes
(global shortcut, keyboard focus, pointer-over target); it does not
re-derive modifiers, track key-repeat, or talk to a device, all of which
`sher_input_core::InputService` already does.

### SHER-Input integration (verified working)

SHER-Input (`~/SHER-Input`, `Mullassery/SHER-INPUT`) is a real, separate repo
now, not a planned future sibling — `sher_display_input` depends on it
directly (`sher_input_core`) and has since dropped its temporary bridge to
SHER-Kernel's `input_driver` entirely. The integration is exercised, not just
declared: `sher_display_input`'s test suite drives a real
`sher_input_core::InputService` through `sher_input_test::SimulatedController`
(a hermetic fake backend SHER-Input itself provides for exactly this purpose)
and asserts on the actual routed output — 5 tests, all passing, no mocks of
SHER-Display's own making.

What `InputRouter` gets for free from consuming the real contract instead of
a stand-in: modifier state (`Modifiers`) computed once by SHER-Input instead
of tracked ad hoc; a canonical `InputEvent` carrying timestamp, sequence
number, device id, and a `source` that always distinguishes physical from
synthetic input; and `CaptureRegistry`/`CaptureGuard` for explicit, single-
owner, revocable pointer capture (spec section 23) — which the earlier
`input_driver`-backed version had no way to express at all. `InputRouter`
holds the one `Arc<InputService>` and its own stream subscription, never a
second copy of device or capture state — the same discipline `outputs`
follows for SHER-Graphics's `GraphicsRuntime`.

### The SHER-Kernel `wayland_server` decision (resolved)

SHER-Kernel used to contain a `WaylandCompositor` — surfaces, buffers,
outputs, pointer/focus, all in the kernel. That violated the boundary above:
surface/output/focus policy is SHER-Display's job. This has already been
resolved:

- SHER-Kernel's `wayland_server` crate now exposes `WaylandTransport` — client
  connection lifecycle and shared buffer handles only. This is retained,
  low-level, kernel-owned.
- The original `WaylandCompositor` struct is marked `#[deprecated]`, kept for
  compatibility, and frozen — no new functionality is added to it.
- SHER-Display's `compatibility/wayland` layer (see Roadmap) consumes
  `WaylandTransport` as its low-level substrate instead of duplicating it.

This is the template for every other kernel/graphics boundary question: when
in doubt, kernel keeps the transport primitive, SHER-Display keeps the
policy.

### The SHER-Graphics `GPUDriver` ownership decision (resolved)

SHER-Graphics's `graphics_runtime` crate already contains a
`PresentationBridge` that owns the one `gpu_driver::GPUDriver` instance used
for connector registration, mode state, and page-flip/present calls
(`GraphicsRuntime::register_connector` / `present_frame`). An early version
of `sher_display_outputs::OutputManager` instantiated its own separate
`GPUDriver` — a real boundary violation: two independent, unsynchronized
owners of the same display hardware, exactly what this document says not to
do.

Resolved the same way as the kernel/wayland_server question: SHER-Graphics
keeps the one hardware-facing instance. `OutputManager` never constructs a
`GPUDriver`; it only mirrors `Connector`/`DisplayMode` facts (already
registered with the real `GraphicsRuntime` by whoever wires SHER-Display to
SHER-Graphics — see Roadmap Phase 3) into desktop-only policy: logical
position, per-output scale, orientation, and which output is primary.
`observe_connector`/`update_mode`/`handle_hotplug` are named to make that
read-only relationship explicit — none of them call into a GPU driver.

### The SHER-Graphics cursor seam (confirmed, not yet wired)

SHER-Graphics has since added real hardware-cursor primitives to
`GraphicsRuntime<D>`: `set_cursor_image`, `set_cursor_position`,
`show_cursor`, `hide_cursor`, `cursor_state`, one `CursorState` per
presentation connector. Its own doc comments state the boundary explicitly:
SHER-Graphics knows *how* to render a cursor efficiently; it has no way to
*derive* a cursor image or position from a raw pointer event — those come
from an external caller that already decided them. That caller is
`sher_display_cursor`.

This is confirmed as the correct seam, not yet wired in: `GraphicsRuntime<D>`
is generic over `D: GpuDriver`, and every cursor method lives in
`impl<D: GpuDriver> GraphicsRuntime<D>`. Wiring `sher_display_cursor` to call
it directly means deciding how SHER-Display parameterizes over the driver
type across the whole compositor stack — a real Phase 3 architectural
decision (see Roadmap), not something to back into as a side effect of
verifying the API exists. `sher_display_cursor::CursorManager` continues to
track policy (`negotiate_render_mode`) as intent for now; the concrete call
site is `GraphicsRuntime::{set_cursor_image, set_cursor_position, show_cursor,
hide_cursor}` once that decision is made.

**Rule of thumb going forward:** if a SHER-Display crate is about to write
`gpu_driver::GPUDriver::new(...)`, `input_driver::InputDriver::new(...)`, or
anything else that constructs a stateful handle to hardware SHER-Kernel or
SHER-Graphics already owns, that's the signal to stop and mirror facts
instead of owning a second copy of the state.

## Display model

```
Display System
   │
   ├── Outputs      — physical/logical display (resolution, scale, refresh rate, position)
   ├── Workspaces    — logical desktop area, independently switchable per output
   ├── Windows       — user-facing top-level object (title, layout, focus, state)
   └── Surfaces      — renderable content object (buffer, damage, commit lifecycle)
                          └── Buffers — actual pixel/storage resource
```

These five concepts stay distinct structures, never collapsed into one. A
window can outlive the surface it currently wraps (e.g. re-parenting during
an XWayland re-map); a surface tree can exist without window semantics at all
(cursors, tooltips, layer-shell-style overlays).

## Protocol and compatibility philosophy

SHER-Display defines its own native client/compositor protocol, designed
around SHER's requirements, not a reimplementation of `wl_display`. Wayland's
concepts are worth studying — surfaces, roles, frame callbacks, XDG shell —
but the protocol is SHER's own.

Compatibility is a boundary, not an architecture:

```
Wayland Client  →  Wayland Compatibility Layer  →  SHER-Display
X11 Application →  XWayland Compatibility Layer →  SHER-Display
```

Never the inverse — SHER-Display's internals must not be permanently modeled
around Wayland semantics just because compatibility is important today.

## Backend abstraction

The core compositor (scene graph, surface/window state machines, damage
tracking, frame scheduling, focus) must not depend on backend details. A
backend is responsible for: display discovery, output enumeration,
presentation, buffer integration, synchronization, display modes, hardware
cursor, and vblank/presentation timing. The first backend targets Linux/Ubuntu
via SHER-Kernel and SHER-Graphics; a native SHER backend follows once
SHER-Kernel/SHER-Graphics no longer need Ubuntu underneath them.

## Non-goals

SHER-Display does not attempt to become: a full desktop environment, a
complete application toolkit, a new GTK replacement, a new graphics API, a
new kernel, a new GPU driver ecosystem, an application package manager, or an
AI model runtime. Those belong to other SHER projects or to Aurora.

## Definition of success

A user can boot Ubuntu into Aurora running on SHER-Display and: see the
desktop, launch applications, create/move/resize/maximize/minimize/fullscreen
windows, switch focus, open menus and popups, use multiple displays with
independent scaling, use keyboard and pointer input, close applications, and
have SHER-Display recover cleanly when an application crashes, a monitor
disconnects, or a GPU error occurs — without taking down the rest of the
desktop.

Longer term, SHER-Display also exposes structured, permission-gated APIs so a
SHER AI/agent layer can inspect and manipulate the graphical session (move a
window, arrange a layout, capture a permitted screenshot) without simulating
mouse and keyboard input, and can run in a headless/virtual-display mode when
no human display is required.
