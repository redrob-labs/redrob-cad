# Redrob CAD

Redrob CAD is an agentic 2D drafting app that lets an AI agent draw, edit and check DWG and DXF
drawings for you.

한국어: [README.ko.md](./README.ko.md)

It is a hard fork of [CADCraft](https://github.com/storytold/cadcraft), written in Rust. It runs natively
on macOS, Windows, Linux and FreeBSD, and in the browser via WebAssembly. Agents drive it over MCP
(`crates/mcp`, see [docs/mcp.md](./docs/mcp.md)).

## Status

Early. The tree is CADCraft `v0.3.0` with our fork files added. Planned next, one pull request each:

1. Rebrand to Redrob CAD (name, app id, icon, about text; upstream trademarks removed).
2. Redrob Code as the built-in agent, talking to the app over MCP.
3. Connect device, so the app signs in to a Redrob workspace.

## Build and test

Requirements: Rust stable (1.90 or newer). On Linux, the X11/Wayland development libraries listed in
[`.github/workflows/ci.yml`](./.github/workflows/ci.yml).

```sh
git clone https://github.com/redrob-labs/redrob-cad.git
cd redrob-cad
cargo run -p cadcraft --release   # the desktop app
cargo xtask ci                    # the gate CI runs
```

## Documents

- Fork point, absorbing upstream, what this fork does not own: [UPSTREAM.md](./UPSTREAM.md)
- Attribution: [NOTICE](./NOTICE)
- Contribution rules: CONTRIBUTING.md and FORKS.md in [redrob-labs/.github](https://github.com/redrob-labs/.github)

## License

MIT OR Apache-2.0, at your option, as upstream. See [LICENSE-MIT](./LICENSE-MIT),
[LICENSE-APACHE](./LICENSE-APACHE) and [NOTICE](./NOTICE).
