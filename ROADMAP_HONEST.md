# ROADMAP_HONEST: technical debt and tooling-gap supplement

`ROADMAP.md` already tracks feature-by-feature status honestly
(`[x]`/`[~]`/`[ ]`) and is the authoritative plan — read that first. This
file is a narrower supplement: technical debt, CI/tooling gaps, and
verification limits found during an OSS-standardization pass on
2026-09-20, none of which `ROADMAP.md` covers. Nothing here duplicates or
contradicts it.

## What was actually verified this pass, and how

- `cargo build --workspace --all-targets` — clean, no errors, no warnings.
- `cargo test --workspace` — **56/56 tests pass**, 0 failures. Matches the
  number README.md already claims; independently re-counted from raw
  `cargo test` output (not taken on faith): 4+4+4+3+3+5+5+4+3+4+6+3+4+4 = 56
  across the 14 workspace crates.
- `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
- `cargo fmt -- --check` — clean.
- `actionlint .github/workflows/ci.yml` — clean, no findings.
- `grep` for `TODO`/`FIXME`/`XXX`/`unimplemented!`/`todo!`/`#[allow(` across
  all `*.rs` — zero matches. Confirms README's "no TODO/FIXME markers"
  claim.
- These checks all ran with `SHER-Kernel`, `SHER-Graphics`, and `SHER-Input`
  present as real sibling directories on the local machine (not stubbed),
  so this is a genuine from-scratch verification, not a claim taken from
  prior commit messages.
- **`cargo audit` — verified for real in a later pass (2026-09-22), network
  was available this time:** fetched the RustSec advisory DB (1258
  advisories), scanned `Cargo.lock`'s 75 crate dependencies, **zero
  vulnerabilities found**, exit code 0. This supersedes the "unverified,
  no network" status below — the `cargo-audit` CI job added in the prior
  pass is confirmed to actually run and pass against real advisory data,
  not just syntactically valid YAML.
- **Still not verified, no network access in that specific session:**
  `gh issue list` (GitHub API timed out — README's "no open GitHub issues"
  claim could not be independently re-confirmed), and dependency-freshness
  check (no `cargo-outdated` installed). Treat those specific claims as
  carried forward from a prior pass, not reconfirmed today.

## Bugs / gaps fixed in this pass

- **`Cargo.lock` was gitignored and never committed, silently degrading
  CI's cache.** `.gitignore` had a bare `Cargo.lock` line; `git ls-files`
  confirmed it was never tracked. `.github/workflows/ci.yml`'s cache step
  keys on `hashFiles('SHER-Display/Cargo.lock')` — since the file didn't
  exist in a fresh CI checkout either (nothing to track means nothing to
  clone), that `hashFiles()` call always returned an empty string, so the
  cache key silently degraded to a constant (`${{ runner.os }}-cargo-`)
  instead of varying with dependency changes. Not a correctness bug (the
  `restore-keys` fallback still works), but it meant CI was never getting
  the cache-invalidation behavior the workflow author intended, and this
  workspace's build was not reproducible from a fresh clone (dependency
  versions could drift between environments). Fixed by removing the
  `.gitignore` entry and committing `Cargo.lock`.
- **Unwrap-after-invariant-check pattern hardened (2026-09-22).**
  `windows/src/lib.rs:126` and `compositor/src/lib.rs:104,113` (see below
  for the original description) now use `.expect("<invariant, spelled
  out>")` instead of bare `.unwrap()`. This doesn't change behavior on any
  currently-passing path — it's still a panic if the invariant is ever
  violated — but a future refactor that breaks the invariant now panics
  with a message identifying exactly which assumption broke, instead of a
  bare "called `Option::unwrap()` on a `None` value". Full restructuring
  around `Entry`/`Result` was judged out of scope for a quick-fix pass
  (each call site's surrounding control flow would need to change, not
  just the failure mode) and is still worth a dedicated look if this code
  sees heavy future refactoring.
- **`cargo audit` run for real (2026-09-22).** Network was available this
  session (it wasn't in the pass that first flagged this as unverified);
  75 dependencies scanned against 1258 RustSec advisories, zero
  vulnerabilities found. See the verification section above for detail.

## Technical debt found, not fixed (needs a dedicated follow-up)

None of the below are bugs today — they're all provably safe by local
invariants, verified by reading the surrounding code — but they're worth a
dedicated pass:

- **Dependency-vulnerability scanning in CI — locally confirmed working,
  real-CI-runner confirmation still outstanding.** The `cargo audit` job
  added in the prior pass (`.github/workflows/ci.yml`) was run for real
  locally on 2026-09-22 (see "What was actually verified" above): 75
  dependencies, 1258 advisories, zero findings, exit 0. That means the
  workspace's loose major-version constraints (`tokio 1`, `anyhow 1`,
  `thiserror 1`, `tracing 0.1`, `tracing-subscriber 0.3`, `serde 1`,
  `serde_json 1`, `uuid 1` — `Cargo.toml:41-47`) resolve to versions with
  no currently-known advisories as of this pass. Still not confirmed
  green on an actual GitHub Actions runner (this repo hasn't been pushed
  from this pass yet) — that's an environment-parity check, not a code
  check, so leaving it for whoever pushes next to watch the Actions tab.
- **Dependency freshness is unverified.** No `cargo-outdated` run (tool not
  installed, needs network regardless). The workspace pins loose
  major-version ranges, not exact versions, so `Cargo.lock` (now committed)
  is the only source of truth for what's actually resolved — worth a
  `cargo update --dry-run` pass with network access.
- **Unwrap-after-invariant-check pattern in production (non-test) code —
  hardened to `.expect()` with explanatory messages (2026-09-22), full
  `Entry`/`Result` restructuring still open:**
  - `windows/src/lib.rs:126` (`activate()`) and `compositor/src/lib.rs:104,
    113` (`tick()`) — see "Bugs / gaps fixed in this pass" above for what
    changed. The underlying fragility (a `contains_key`/`keys().collect()`
    check that the borrow checker doesn't tie to the later `get_mut()`)
    still exists structurally; only the failure mode improved, from a bare
    panic to a panic with a message naming the broken invariant. None of
    these are reachable from external input today (no network, no real
    hardware) so there's no live severity. A dedicated pass restructuring
    around the `Entry` API (or returning `Result` all the way through
    `tick()`) would close the fragility itself, not just improve its
    failure message — still worth doing, just out of scope for this
    quick-fix pass since it touches control flow, not just the unwrap
    call.
- **Coverage gaps already called out honestly in `ROADMAP.md` but worth
  restating as debt, not just missing features:** touch/tablet/gamepad
  input routing (`input/src/lib.rs`, Phase 4) is wired but has "not yet
  exercised by a dedicated test" per `ROADMAP.md` — meaning it currently
  ships with zero direct test coverage despite being reachable code, unlike
  every other routing path in the same file. Buffer release/fence semantics
  (`surfaces::SurfaceState.buffer_id`, Phase 1) are flagged as
  entirely unimplemented, which also means zero coverage exists to
  regress against once someone starts that work.
- **`security` and `session` crates model policy, not enforcement.**
  `security/src/lib.rs`'s permission grants and `session/src/lib.rs`'s
  login/lock state machine are in-memory `HashMap`-backed structures with
  no connection to a real process boundary, real clock source beyond
  whatever `SystemTime`/`Instant` the caller passes in, or real
  authentication. This is explicitly disclosed in `SECURITY.md` (added this
  pass) — flagging here too so it isn't only visible to someone who reads
  that file.
- **Local filesystem artifact, not a repo issue:** the working directory on
  this machine has empty, git-untracked `src/` subdirectories under `ai/`,
  `animation/`, `dragdrop/`, `headless/`, `recording/`, `screenshot/`,
  `tests/`, and `clipboard/` (confirmed via `git ls-tree -r HEAD` — none of
  these paths are tracked; only `clipboard/Cargo.toml` is). These don't
  exist in a fresh clone and don't affect the committed repo state, so
  they're not fixed here, but noting them in case they're confusing to
  find locally.

## Architecture-boundary audit (re-verified this pass)

Re-ran the boundary check the README already claims (`grep` for
`GPUDriver`/`InputDriver`/other driver-construction patterns across all
`*.rs`):

```
grep -rn "GPUDriver::new\|InputDriver::new" --include='*.rs' .
```

Zero matches outside of comments/doc references in `VISION.md` (not `.rs`
files). **No architecture-boundary violation found** — `outputs/` does not
instantiate `gpu_driver::GPUDriver`, and no crate in this repo constructs a
driver/hardware handle owned by SHER-Kernel or SHER-Graphics. This confirms
(does not merely repeat) the claim in README.md's "Cross-repo compatibility"
section and VISION.md's "GPUDriver ownership decision."

## What this pass added

- `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`,
  this file.
- `.github/ISSUE_TEMPLATE/bug_report.yml`, `.github/ISSUE_TEMPLATE/feature_request.yml`,
  `.github/pull_request_template.md`.
- `.github/dependabot.yml` (cargo ecosystem, weekly).
- A `cargo-audit` CI job (locally verified against real advisory data on
  2026-09-22 — see above; GitHub Actions runner confirmation still
  outstanding).
- `docs/architecture/README.md` with Mermaid diagrams of the cross-repo
  layering and the internal crate layering — no architecture doc existed
  before this pass.
- Committed `Cargo.lock`, removed it from `.gitignore`.

## What this pass deliberately did not do

(Original 2026-09-20 documentation-first pass — the first two bullets below
were later revisited and fixed in the 2026-09-22 quick-fix pass; see "Bugs
/ gaps fixed in this pass" above.)

- ~~Did not fix the unwrap-after-invariant-check pattern above~~ — hardened
  to `.expect()` with explanatory messages on 2026-09-22 (full `Entry`
  restructuring still open, see above).
- ~~Did not attempt to verify `cargo audit`~~ — run for real on 2026-09-22,
  network was available; zero vulnerabilities found (see above).
- Did not execute the Phase 0 structural migration to `crates/` layout —
  it's an open decision in `ROADMAP.md`, not something to force through a
  standardization or quick-fix pass.
- Did not add touch/tablet/gamepad or buffer-release tests — that's new
  feature work belonging to Phase 1/4, not a docs/tooling/quick-fix pass.
- Did not build real enforcement into `security`/`session` — that's new
  feature work, not a quick-fix.
- Did not attempt to verify `gh issue list` or dependency freshness against
  crates.io — no network access for those specific checks in this session
  (unrelated to the `cargo audit` fetch, which did succeed). Flagged above
  as still worth a follow-up with GitHub API access restored.
