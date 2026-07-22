# guiltty

A Rust library for drawing real, pixel-level 2D graphics into a terminal, using the
[kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/) as its first
backend.

## Why

Character-cell TUI toolkits (`ratatui`, `cursive`) can't express real images, smooth shapes,
or precise positioning — they're built around a grid of character cells. Kitty's graphics
protocol can draw actual pixels into a compatible terminal, the way
[`kui.nvim`](https://github.com/romgrk/kui.nvim) demonstrated — but that project is stale and
locked to Neovim as a host. guiltty aims at the same capability as a standalone,
host-independent Rust library.

When finished, a Rust developer will be able to create a canvas, draw text and shapes into
it, place and move sprites over a preserved background, carve out multiple independent
viewport regions within one terminal, zoom a canvas, and work with canvases larger than the
visible terminal — all rendered live via the kitty graphics protocol.

## Status: early scaffold, pre-alpha

This project is in the earliest stage of development. What exists today:

- A three-crate Cargo workspace (`guiltty-core`, `guiltty-kitty`, `guiltty`) matching the
  architecture described in [`docs/spec.md`](docs/spec.md).
- `guiltty-core`: the `Color` (RGBA8), `Point`, and `Rect` primitives, plus the backend-agnostic
  `Backend` trait that rendering backends implement.
- `guiltty-kitty`: a `KittyBackend` scaffold implementing `Backend` — no actual escape-sequence
  encoding or terminal transmission yet.
- `guiltty`: a facade crate re-exporting the above.

**None of the actual drawing functionality exists yet** — no `Canvas`, no shapes, no sprites,
no viewport regions, no zoom/scroll, and no real kitty-protocol encoding. See
[`docs/spec.md`](docs/spec.md)'s Success Criteria for what v0 will include once built, and
[`docs/intent/kitty-graphics-ui-toolkit.md`](docs/intent/kitty-graphics-ui-toolkit.md) for the
confirmed project intent this spec implements.

This is a solo project (author + AI coding agents) with no fixed deadline.

## Building

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Rust toolchain is pinned via [mise](https://mise.jdx.dev/) (see `mise.toml`).

## License

MIT — see [`LICENSE`](LICENSE).
