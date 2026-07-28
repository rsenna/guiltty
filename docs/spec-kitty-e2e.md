# Spec: kitty-based E2E verification

## Objective

Every kitty-protocol-facing task so far (T1's `KittyBackend::present()`, T4's
runnable example) has shipped with the same unresolved caveat: *manual
real-terminal visual verification is still pending — no kitty-compatible
terminal available in this environment.* Nothing built so far has ever been
confirmed to work against a real terminal. This spec closes part of that gap:
automated verification against a real terminal, using
[kitty](https://sw.kovidgoyal.net/kitty/) itself — the protocol's reference
implementation — as a black-box test oracle.

**Why kitty specifically, not a reimplementation:** this spec originally
targeted WezTerm, a third-party reimplementation of the kitty graphics
protocol. WezTerm has a documented history of protocol-compatibility gaps
with kitty's own implementation — exactly the kind of divergence that could
mask or fabricate bugs in either direction (WezTerm rendering something kitty
wouldn't, or rejecting something kitty accepts). Since kitty is the reference
implementation, testing directly against it removes that ambiguity for the
questions that matter most: does *the terminal this protocol was designed
for* accept and correctly handle what we send.

**Why now:** T1 and T4 are both implemented and gate-green, but the "does a
real terminal even accept this?" question has been open since T1's first PR.
Two independent code reviews on the kitty-encoding PRs already found real
protocol bugs (missing `q=`/`C=`/`i=` keys, a kittage double-terminator bug,
an image-id collision) purely from static analysis — a real terminal in the
loop would catch classes of bug static review can't.

## Researched and ruled out

Before settling on this approach, two other mechanisms were investigated and
rejected based on kitty's actual documented capabilities (via kitty's own
docs, not assumption):

- **Reverse transmission** (asking the terminal to hand back the pixel data
  it has stored/rendered for a given image, to compare against what was
  sent): not supported. The kitty graphics protocol's full action set is
  `t`/`T` (transmit / transmit-and-display), `p` (place), `d` (delete), `q`
  (query), `c` (compose animation frame) — every one of these is
  client→terminal only. `a=q` returns only an `OK`/error status, never pixel
  data. There is no protocol-level path to read image data back out.
- **Clipboard-based capture** (kitty's `kitten clipboard` / OSC 5522
  extension does support copying images to/from the system clipboard): real
  feature, but it's general-purpose clipboard I/O — push a local file to the
  clipboard, or read whatever the *system* clipboard currently holds to a
  file. No evidence of a feature that captures "what the graphics protocol
  currently has displayed on screen" onto the clipboard. Using it to push our
  own known-good source bytes to the clipboard wouldn't verify anything about
  whether the actual graphics-protocol transmission rendered correctly — it
  would only prove the unrelated OSC 5522 clipboard write path works.

## Scope: two verification tiers

1. **Protocol-acceptance testing (in scope for this spec).** Prove that
   kitty itself parses guiltty's escape sequences without an error response.
   kittage's `Verbosity::ErrorsOnly` (or `All`) instead of `Silent` makes the
   terminal respond, and kittage's `Action::execute` (rather than
   `write_transmit_to`) already reads and parses that response via its
   `InputReader` trait.
2. **Pixel-level visual verification (elevated from "open question" to
   "plausible," pending prototyping).** Unlike WezTerm's genuinely
   GPU/display-free mux server, kitty has no equivalent headless daemon: it's
   a single GPU-rendering process, and `--start-as=hidden` only hides the
   window — it doesn't remove the need for a display server. Running kitty
   headlessly for CI means running it under a **virtual** display (Xvfb) with
   software GPU rendering (e.g. via Mesa's llvmpipe). The upside: a real
   (virtual) X11 framebuffer is a real thing standard screenshot tools
   (`xwd`, `import`, `scrot`, `ffmpeg` capturing the X display) can capture —
   something WezTerm's deliberately renderer-free mux server never offered.
   This tier needs prototyping to derisk (confirm software rendering is
   stable enough to trust for pixel comparison, confirm screenshot timing
   relative to when kitty finishes rendering a frame) before being promoted
   to a Success Criterion.

## Tech Stack / Approach

- **Binary-only, black-box dependency.** kitty is a **dev-dependency only**
  — never a build or runtime dependency of `guiltty`/`guiltty-core`/
  `guiltty-kitty`. No `Cargo.toml` entry for any kitty-related Rust crate;
  guiltty's own dependency graph is entirely unaffected by this spec. kitty
  is consumed exclusively as a prebuilt binary, invoked as an external
  subprocess, the same way `cargo-llvm-cov` or any other dev tool is used.
- **Provisioning.** kitty publishes official signed prebuilt binaries: the
  full terminal for Linux (x86_64/arm64, `.txz`) and macOS (`.dmg`); for
  BSDs, only the companion `kitten` CLI tool is officially published
  (FreeBSD/DragonFly/NetBSD/OpenBSD, amd64/arm64) — **not** the full terminal
  binary, so BSD would need the platform's own package manager (e.g.
  FreeBSD's `pkg install kitty` / ports `x11/kitty`) for the actual
  terminal, same gap as WezTerm had. Checked `mise`'s plugin registry
  previously for WezTerm and found no entry; same check needed for kitty
  before committing to a custom download script.
- **Two distinct channels, not one** — a gap in an earlier draft of this
  spec that review caught: kitty's remote-control socket
  (`--listen-on unix:<path>`) carries *kitty control commands only*
  (spawning/managing windows, querying state) — it is **not** how graphics
  protocol bytes reach kitty or how kitty's graphics responses get read
  back. Those travel over the PTY of whichever window is actually running
  the harness process, i.e. the harness's own stdout/stdin once kitty has
  spawned it. Concretely: the control socket is used only to issue a
  `launch` remote-control action that spawns a new kitty window running the
  harness binary; the harness then talks graphics protocol directly over
  its own stdout (which is that window's PTY) and reads kitty's response
  from its own stdin — no different from how the harness would work
  outside any orchestration at all.
- **Headless orchestration.** Xvfb (or an equivalent virtual framebuffer) +
  kitty configured for software rendering, launched with
  `allow_remote_control=socket-only --listen-on unix:<unique-per-run-path>`
  (a fresh path under a temp directory for every test invocation, e.g.
  including the test's own process id) so an orchestrating process can issue
  the `launch` action (see above) to spawn the harness inside a real kitty
  window, without needing interactive keyboard/mouse input. The unique
  socket path matters: a fixed/well-known path risks the harness attaching
  to an unrelated kitty instance already running on the dev machine, or two
  concurrent test runs colliding with each other (flagged independently by
  two reviewers on this PR's earlier WezTerm-based draft — the same
  underlying risk applies here).
- **Test harness.** A small, dedicated binary, spawned inside a real kitty
  window via the `launch` remote-control action above (not connected to the
  remote-control socket itself), that: builds a `Canvas` exercising the
  feature under test, presents it using `Verbosity::ErrorsOnly` instead of
  `Silent`, reads kitty's response from its own stdin (its window's PTY),
  and writes a plain PASS/FAIL result directly to a **unique temporary file
  path** (passed to the harness as an argument, not a fixed well-known
  path — avoids race conditions between parallel test runs and false
  positives from a stale result file left over from a previous run) — not
  through kitty's own text-capture remote-control actions, since APC
  responses are program-visible (read by our own process from its stdin),
  not part of on-screen "text" content a `get-text`-style action would
  capture.
- **Orchestrating test.** A `#[test]`, `#[ignore]`'d by default (needs the
  kitty binary and Xvfb present, isn't part of the fast unit-test loop), that
  starts Xvfb + kitty, spawns the harness, polls the result file against a
  **hard deadline** (fails rather than hanging indefinitely if no response
  ever arrives — kitty's response could in principle never come), and tears
  everything down **unconditionally** — an RAII-style process guard (kill
  Xvfb/kitty on `Drop`) rather than only tearing down on the test's success
  path, so a panic or early-return doesn't leak the Xvfb/kitty processes,
  the temp socket, or the temp result file. Run explicitly or from a
  dedicated CI job — never folded into the existing
  fmt/clippy/test/90%-coverage gate, which must stay fast and independent of
  external binaries/a virtual display.

## Boundaries

- **Always:** treat kitty (and Xvfb) as dev-only tooling; verify downloaded
  binaries against their published signatures before use; keep this test
  suite fully separate from (and non-blocking to) the existing quality gate.
- **Ask first:** the specific kitty version to pin; the exact provisioning
  mechanism (pinned download script vs. any `mise` entry, if one exists);
  whether/how to pursue the Tier 2 (pixel-level) screenshot prototype once
  Tier 1 is working, given it needs real derisking work first.
- **Never:** add kitty as a build or runtime dependency of any guiltty
  crate; build kitty from source as part of this project's own build/test
  process; vendor kitty's source; make this E2E suite a required check that
  blocks the existing fast quality gate.

## Success Criteria

1. A documented, scripted way to fetch a pinned, signature-verified kitty
   release binary for Linux and macOS without building from source (BSD:
   documented as "use the platform's package manager instead," matching the
   same gap identified for WezTerm).
2. A headless (Xvfb-backed) kitty instance that a test harness can drive via
   remote control, presenting a guiltty `Canvas` and capturing whether the
   kitty graphics protocol commands were accepted (OK) or rejected (error).
   **Linux only**: Xvfb is X11-specific and has no macOS equivalent (macOS
   uses Quartz, not X11) — this automated tier does not cover macOS. macOS
   verification stays manual (Success Criterion 4), same as BSD.
3. At least one `#[ignore]`'d automated test exercising this against T1/T4's
   existing `present()` path, runnable on demand, separate from the default
   `cargo test --workspace` gate.
4. A documented manual procedure for actual pixel-level visual confirmation
   using a real, non-virtualized kitty instance — replacing "no terminal
   available in this environment" with a concrete, run-it-yourself set of
   steps, regardless of whether the Tier 2 automated screenshot path pans
   out.

## Open Questions

- Whether the Tier 2 screenshot approach (Xvfb + software rendering +
  standard X11 screenshot tooling) is stable/reliable enough to trust for
  automated pixel comparison — needs a prototyping spike before committing
  to it as a Success Criterion rather than a stretch goal.
- Exact provisioning mechanism: whether `mise`'s registry has (or gains) a
  kitty entry, vs. a custom pinned-download script.
- CI platform coverage: the automated Xvfb-backed tier-1 suite is **Linux
  only** (Xvfb is X11-specific, no macOS equivalent — GitHub Actions macOS
  runners can't use it), even though guiltty itself targets macOS and BSDs
  too. Whether a macOS-native headless/capture path exists (some other
  virtual-display or screen-capture mechanism) is unexplored — for now,
  macOS and BSD verification both stay manual, local-only procedures.
- Whether the harness should use kittage's synchronous `Action::execute`
  (blocks on reading the terminal's response) or a different read strategy —
  needs prototyping once implementation starts.
- Exact kitty version to pin, and where that pin lives — deferred to the
  implementation task per the Boundaries' "ask first" note above.
