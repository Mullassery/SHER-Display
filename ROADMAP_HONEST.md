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
- **Not verified this pass, sandbox has no network access to
  github.com/crates.io:** `cargo audit` (RustSec advisory DB fetch timed
  out), `gh issue list` (GitHub API timed out — README's "no open GitHub
  issues" claim could not be independently re-confirmed this pass), and any
  dependency-freshness check (no `cargo-outdated` installed, and it needs
  network regardless). Treat those specific claims as carried forward from
  a prior pass, not reconfirmed today.

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

## Technical debt found, not fixed (needs a dedicated follow-up)

None of the below are bugs today — they're all provably safe by local
invariants, verified by reading the surrounding code — but they're worth a
dedicated pass:

- **No dependency-vulnerability scanning in CI.** Added a `cargo audit` job
  in this pass (`.github/workflows/ci.yml`), but it has never actually run
  successfully anywhere — this sandbox can't reach RustSec's advisory DB to
  test it, and it hasn't executed in real CI yet either (this commit hasn't
  been pushed). **Next session must confirm it runs green on the actual
  GitHub Actions runner before trusting it**, and check whether the
  workspace's dependencies (`tokio 1`, `anyhow 1`, `thiserror 1`,
  `tracing 0.1`, `tracing-subscriber 0.3`, `serde 1`, `serde_json 1`,
  `uuid 1` — all major-version-only constraints in `Cargo.toml:41-47`) have
  any known advisories. Not checked, not assumed clean.
- **Dependency freshness is unverified.** No `cargo-outdated` run (tool not
  installed, needs network regardless). The workspace pins loose
  major-version ranges, not exact versions, so `Cargo.lock` (now committed)
  is the only source of truth for what's actually resolved — worth a
  `cargo update --dry-run` pass with network access.
- **Unwrap-after-invariant-check pattern in production (non-test) code,
  correct today but fragile to future refactors:**
  - `windows/src/lib.rs:126` — `self.windows.get_mut(id).unwrap().active =
    true;` inside `activate()`. Safe only because line 117
    (`if !self.windows.contains_key(id) { return Err(...) }`) already
    guarded it a few lines earlier — a `contains_key`-then-`get_mut` split
    that the borrow checker doesn't tie together, so a future edit that
    reorders or removes the early check would turn this into a panic
    instead of a `Result::Err`. Same shape of risk applies to using the
    `Entry` API instead.
  - `compositor/src/lib.rs:104` and `compositor/src/lib.rs:113` — two
    `self.schedules.get_mut(&output_id).unwrap()` calls inside `tick()`,
    both safe because `output_ids` on line 100 is collected directly from
    `self.schedules.keys()` moments earlier and nothing removes entries
    from `schedules` inside the loop. Same fragility: correct only as long
    as nobody adds an early-return or a schedule-removal call between the
    `keys().collect()` and the loop body.
  - None of these are reachable from external input today (no network, no
    real hardware) so there's no live severity — but they're exactly the
    kind of invariant that a well-intentioned refactor breaks silently
    because the compiler won't catch it. Recommend replacing with
    `.expect("<the invariant, spelled out>")` at minimum so a future panic
    at least explains itself, or restructuring around `Entry`.
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
- A `cargo-audit` CI job (unverified — see above).
- `docs/architecture/README.md` with Mermaid diagrams of the cross-repo
  layering and the internal crate layering — no architecture doc existed
  before this pass.
- Committed `Cargo.lock`, removed it from `.gitignore`.

## What this pass deliberately did not do

- Did not fix the unwrap-after-invariant-check pattern above — both sites
  are correct today; a refactor risks introducing new bugs and this is
  explicitly a documentation-first pass, not a fix-everything pass.
- Did not execute the Phase 0 structural migration to `crates/` layout —
  it's an open decision in `ROADMAP.md`, not something to force through a
  standardization pass.
- Did not add touch/tablet/gamepad or buffer-release tests — that's new
  feature work belonging to Phase 1/4, not a docs/tooling pass.
- Did not attempt to verify `cargo audit` actually finds (or doesn't find)
  advisories, or check dependency freshness against crates.io — no network
  access in this sandbox. Flagged above as the first thing to check with
  network access restored.
