# Redrob CAD

Redrob CAD is an agentic 2D drafting app that lets an AI agent draw, edit and check DWG and DXF
drawings for you.

한국어: [README.ko.md](./README.ko.md)

It is written in Rust and runs natively on macOS, Windows and Linux. Agents drive it over MCP
(`crates/mcp`, see [docs/mcp.md](./docs/mcp.md)).

## Status

Early. App id `ai.redrob.cad`, binaries `redrob-cad` and `redrob-cad-cli`. Planned next, one pull
request each:

1. Redrob Code as the built-in agent, talking to the app over MCP.
2. Connect device, so the app signs in to a Redrob workspace.

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

- Code origin and attribution: [NOTICE](./NOTICE)
- Contribution rules: CONTRIBUTING.md in [redrob-labs/.github](https://github.com/redrob-labs/.github)

## License

MIT OR Apache-2.0, at your option. See [LICENSE-MIT](./LICENSE-MIT),
[LICENSE-APACHE](./LICENSE-APACHE) and [NOTICE](./NOTICE).
