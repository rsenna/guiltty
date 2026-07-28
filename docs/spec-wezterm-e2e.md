# Spec: WezTerm-based E2E verification

## Objective

Every kitty-protocol-facing task so far (T1's `KittyBackend::present()`, T4's
runnable example) has shipped with the same unresolved caveat: *manual
real-terminal visual verification is still pending — no kitty-compatible
terminal available in this environment.* Nothing built so far has ever been
confirmed to work against a real terminal. This spec closes part of that gap:
automated, headless, CI-capable verification that a real kitty-protocol
terminal implementation *accepts* guiltty's escape sequences, using
[WezTerm](https://wezterm.org/) as a black-box test oracle.

**Why WezTerm specifically:** it's cross-platform (Linux, macOS, and — via the
platform's own package manager rather than an official prebuilt binary —
BSDs), implements the kitty graphics protocol, is actively maintained, and,
critically, its multiplexer (mux) layer can run **fully headless**: WezTerm's
own docs confirm `wezterm.gui` (the rendering/windowing module) is
unavailable to the mux server, meaning `wezterm-mux-server` is a pure
PTY/pane/state multiplexer with no GPU or display-server dependency. That
makes it viable for CI, not just local dev-machine testing.

**Why now:** T1 and T4 are both implemented and gate-green, but the "does a
real terminal even accept this?" question has been open since T1's first PR.
Two independent code reviews on the kitty-encoding PRs already found real
protocol bugs (missing `q=`/`C=`/`i=` keys, a kittage double-terminator bug,
an image-id collision) purely from static analysis — a real terminal in the
loop would catch classes of bug static review can't.

## Scope: two verification tiers

1. **Protocol-acceptance testing (in scope for this spec).** Prove that a
   real kitty-protocol terminal implementation parses guiltty's escape
   sequences without an error response. This is concretely achievable
   headlessly: kittage's `Verbosity::ErrorsOnly` (or `All`) instead of
   `Silent` makes the terminal respond, and kittage's `Action::execute`
   (rather than `write_transmit_to`) already reads and parses that response
   via its `InputReader` trait. This tier does **not** confirm the image
   *looks* correct pixel-for-pixel — only that the terminal accepted it.
2. **Pixel-level visual verification (explicitly out of scope / open
   question).** Actually confirming rendered output looks correct would need
   either a GUI-attached WezTerm client plus OS-level screenshot tooling, or
   a WezTerm capability this spec's research didn't find evidence of (no
   `screenshot`/pixel-capture CLI command turned up in WezTerm's documented
   CLI surface). Manual visual verification remains the process for this
   tier in v0; automating it is an open question below, not a blocker for
   tier 1.

Tier 1 alone is a meaningful upgrade: it moves "does this work at all against
a real terminal" from *manual, unverified, environment-dependent* to
*automated and checked on every run where the harness is available*.

## Tech Stack / Approach

- **Binary-only, black-box dependency.** WezTerm is a **dev-dependency only**
  — never a build or runtime dependency of `guiltty`/`guiltty-core`/
  `guiltty-kitty`. No `Cargo.toml` entry for any WezTerm Rust crate, ever;
  guiltty's own dependency graph is entirely unaffected by this spec. WezTerm
  is consumed exclusively as a prebuilt binary, invoked as an external
  subprocess (`std::process::Command`), the same way `cargo-llvm-cov` or any
  other dev tool is used.
- **Provisioning.** Checked `mise`'s plugin registry — it has no WezTerm
  entry — so binary provisioning needs its own mechanism rather than
  `mise.toml`. Official prebuilt release binaries exist for Linux (per-distro
  `.deb`/`.tar.xz`/a distro-agnostic `.AppImage`) and macOS (`.zip`, a signed
  `.app` bundle) from WezTerm's GitHub Releases, checksummed (`.sha256` files
  ship alongside every asset). For BSDs, no official prebuilt binary is
  published — the platform's own package manager (e.g. FreeBSD's `pkg install
  wezterm`) is the "binary-only" path there instead of GitHub Releases.
- **Headless orchestration.** `wezterm-mux-server` started as a background
  process (see `daemon_options` for pid-file/log-file locations); `wezterm
  cli spawn` runs a test harness binary inside a real pane it manages —
  confirmed to require no GUI or display server for this operation.
- **Test harness.** A small, dedicated binary (not the T4 demo itself, though
  it can reuse the same `Canvas`-building code) that: builds a `Canvas`
  exercising the feature under test, presents it using `Verbosity::ErrorsOnly`
  instead of `Silent`, reads the terminal's response, and writes a plain
  PASS/FAIL result **directly to a file on the host filesystem** — not
  through WezTerm's own `wezterm cli get-text`, since APC responses are
  program-visible (read by our own process from its stdin), not part of
  what's displayed on-screen; `get-text` captures screen/cell content, which
  doesn't include this out-of-band protocol response at all.
- **Orchestrating test.** A `#[test]`, `#[ignore]`'d by default (since it
  needs the WezTerm binary present and isn't part of the fast unit-test
  loop), that starts the mux server, spawns the harness, polls the result
  file, asserts PASS, and tears the mux server down. Run explicitly (e.g.
  `cargo test --workspace -- --ignored`) or from a dedicated CI job — never
  folded into the existing fmt/clippy/test/90%-coverage gate, since that gate
  must stay fast and independent of an external binary's availability.

## Boundaries

- **Always:** treat WezTerm as dev-only tooling; verify downloaded binaries
  against their published `.sha256` checksums before use; keep this test
  suite fully separate from (and non-blocking to) the existing quality gate.
- **Ask first:** the specific WezTerm version to pin (reproducibility vs.
  staying current); the exact provisioning mechanism if a cleaner option than
  a pinned-download script turns up during implementation (e.g. if `mise`
  gains a registry entry, or if Homebrew/apt pinning is preferred over raw
  GitHub Releases for local dev).
- **Never:** add WezTerm as a build or runtime dependency of any guiltty
  crate; build WezTerm from source as part of this project's own build/test
  process; vendor WezTerm's source; make this E2E suite a required check
  that blocks the existing fast quality gate.

## Success Criteria

1. A documented, scripted way to fetch a pinned, checksum-verified WezTerm
   release binary for Linux and macOS without building from source (BSD:
   documented as "use the platform's package manager instead").
2. A headless `wezterm-mux-server`-based harness that spawns a guiltty-built
   `Canvas`'s `present()` output inside a real WezTerm-managed pane and
   captures whether the kitty graphics protocol commands were accepted (OK)
   or rejected (error) — with no GUI or display server involved.
3. At least one `#[ignore]`'d automated test exercising this against T1/T4's
   existing `present()` path, runnable on demand, separate from the default
   `cargo test --workspace` gate.
4. A documented manual procedure for actual pixel-level visual confirmation
   (attach a real WezTerm GUI client to the same mux session) — replacing
   "no terminal available in this environment" with a concrete, run-it-
   yourself set of steps.

## Open Questions

- Exact pixel-level automated visual verification mechanism, if one turns
  out to exist or becomes available later (no WezTerm-native screenshot/
  pixel-capture capability was found during this spec's research) — tier 2,
  not blocking.
- CI platform coverage: GitHub Actions runners are Linux/macOS/Windows, so
  the automated tier-1 suite is Linux/macOS only in CI even though guiltty
  itself targets BSDs too; BSD verification stays a manual, local-only
  procedure.
- Whether the harness should use kittage's synchronous `Action::execute`
  (blocks on reading the terminal's response) or a different read strategy —
  needs prototyping once implementation starts.
- Exact WezTerm version to pin, and where that pin lives (a new `mise.toml`
  entry once/if one becomes available, a version file, or embedded directly
  in the provisioning script) — deferred to the implementation task per the
  Boundaries' "ask first" note above.
