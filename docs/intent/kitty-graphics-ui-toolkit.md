# guiltty — Confirmed Intent

- **Outcome:** A Rust library that draws real pixel-level 2D graphics into a terminal via the kitty graphics protocol — canvas, text, shapes, sprites, and independent drawable viewport regions — going beyond what character-cell TUI toolkits (ratatui, cursive) can express.
- **User:** The author, solo + AI agents, building it primarily to consume from another Rust project already in progress; a public, more broadly usable crate is the longer-term shape, but not the immediate driver.
- **Why now:** Character-cell toolkits can't do real images/shapes/precise positioning; kitty's graphics protocol can. `kui.nvim` (https://github.com/romgrk/kui.nvim) proved the idea works but is stale and Neovim-locked — the goal is the same power as a standalone, host-independent library.
- **Success (v0/MVP):**
  - (a) Canvas object supporting text drawing
  - (b) Basic shapes: lines, rects, circles, ellipses, triangles, arbitrary open/closed paths, fill
  - (c) Sprites: movable 2D bitmaps over a preserved background
  - (d) Multiple independent viewport regions within one terminal, each drawable with text + shapes
  - (e) Per-canvas zoom levels
  - (f) Canvases larger than the current terminal viewport (content can exceed visible bounds — requires some form of scrolling/panning/clipping to handle varying terminal sizes with "grace")
- **Constraint:** Solo + agents, no deadline. Targets terminals implementing the kitty graphics protocol.
- **Out of scope (v0):** Mouse/event handling, interactive widgets (buttons, clickable elements), and a scriptable CLI binary interface for non-Rust languages (bash, etc.) — deferred to later phases. Eventually the project wants both a Rust library and a `gum`-style standalone binary callable from scripts, but only the Rust library is in scope for v0.
