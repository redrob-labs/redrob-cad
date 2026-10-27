# Redrob CAD — instructions for agents

Redrob CAD is a clean-room, open-source, pure-Rust computer-aided design and drafting application targeting Autodesk AutoCAD parity — and superiority (speed, openness, agent control). It runs natively on macOS, Windows and Linux.

## Start every session here
1. Read `plan/STATUS.md` (current milestone, next task), then the task in `plan/execution-plan.md` and the relevant `plan/architecture.md` section. Behaviour reference: `plan/autocad/*` (`01-observed-ui.md` holds measured observations of the running reference app; `04-menu-tree.txt` the menu tree by name).
2. Library decisions: `plan/adr/0001-geometry-libraries.md` (what we may depend on, what is rejected for licence reasons).
3. Follow the autonomous operation protocol (`plan/execution-plan.md` §6). Don't stop to ask unless it lists the decision as the owner's.

`plan/` is gitignored (local only).

## Never crash
People trust Redrob CAD with their drawings; a crash loses their work. **This outranks feature work.**
- **No panics in non-test code:** no `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`; no `unsafe` (`unsafe_code = "forbid"`).
- **Errors are `Result<T, E>`** through the crate's error type and `?`. An unfinished feature returns an error or reports "not available yet"; it never panics.
- **Input-derived numbers are hostile** (DXF/DWG files, command-line text, MCP/control params): `get()` not `[i]`, checked arithmetic, no NaN casts, cap input-sized allocations and loop counts.
- **Bound recursion** (nested/cyclic block references: `MAX_BLOCK_DEPTH`).
- **Last-resort guard:** `Session::execute` and interactive command input run under `catch_unwind`; an escaped panic restores the drawing and reports an error.
- **Prove it:** every crash fix lands with a regression test (see `hostile_params_never_panic`, `hostile_dxf_does_not_panic`).
- Every production crate root carries `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`.

## Non-negotiables
- **Clean-room.** AutoCAD is installed on the dev machine and may be *observed* black-box: run it, use its UI with synthetic drawings, take screenshots (by window id) stored only under `plan/autocad/screenshots/` (never committed). **Never** read, disassemble or copy anything inside the AutoCAD bundle (names/listings only); never copy Autodesk fonts (SHX), hatch patterns (`.pat`), linetypes (`.lin`), templates, CUI/PGP files, icons, artwork or help text; never commit files produced by AutoCAD. File formats come from public specifications (Autodesk's published DXF Reference, the Open Design Alliance's public DWG specification) and our own synthetic tests. Never copy or link GPL/LGPL/AGPL code (LibreDWG, LibreCAD, QCAD, FreeCAD's planegcs, SolveSpace, OpenCASCADE) — don't even read their sources.
- **Assets — absolutely essential.** Redrob CAD contains **no Autodesk, Adobe or Avid iconography, images, fonts, patterns or artwork — ever.** Every icon is drawn in code (`crates/ui-egui/src/icons.rs`), every hatch pattern and linetype is our own definition (`crates/doc/src/library.rs`), the drafting font is our own (`crates/fonts/src/stroke.rs`). Any file asset must be original, public domain/CC0, OSI-licensed, or redistributable Creative Commons (or licensed open source by a contributor who made it), and **must have a row in `ATTRIBUTION.md`** with author, source and licence (`cargo xtask assets` enforces it). The only exception is the Redrob mark in `assets/brand/`, copied verbatim from the Redrob design system and pinned by `assets/brand/SHA256SUMS`. Screenshots of Autodesk software are never committed. Breaking this rule is the most serious mistake you can make in this repo.
- **Fonts live in [`storytold/craft-fonts`](https://github.com/storytold/craft-fonts)**, never in this repo. It is an optional build input (`REDROB_CAD_FONTS_DIR`), never a `Cargo.toml` dependency. Code and tests must work without it.
- **Shared test corpora** live in separate repositories; never commit large binary fixtures here.
- **Everything is a command.** User-visible behaviour = a command in `crates/engine/src/cmd/*` (`CommandSpec`: id = AutoCAD's command name in lower case, label, menu path, shortcut, aliases, params doc, `enabled`, JSON `run`, optional `interactive` prompt machine) + tests. UI-only commands live in `crates/ui-egui/src/menus.rs` (`UI_COMMANDS`). The command line, menus, toolbar, Tool Sets, scripts, CLI, control channel and MCP all reach the same commands.
- **Programmatic calls never open dialogs.** `engine.execute` runs the JSON form with defaults; only menu/toolbar invocation starts the interactive prompt sequence.
- **Layering** is enforced by `cargo xtask layers`: L0 `geom`, `dxf` → L1 `color`, `doc` → L2 `fonts`, `render`, `constraints` → L3 `io` → L4 `engine` → L5 `ui-egui`, `mcp` → apps. Nothing below L5 depends on egui/eframe/winit/wgpu/rfd. **The UI crate is swappable.**
- **The UI is thin**: panels read engine state and act through `app.run(id, params)` / `app.start(id)` / `app.cmdline(text)`. Colours come from `theme::Tokens`.
- **Rust only** (no handwritten JS/TS). Desktop only: no web/wasm build.
- **Quality gates** before every commit: `cargo xtask ci` (fmt, clippy -D warnings, tests, assets, brand, layers). Commit after every feature arc that builds, and push to `main`.

## Running and looking at the app
- `cargo run --release -p redrob-cad -- --sample --control PORT` (sample drawing + control channel). Pick a free port; other apps may use control ports too.
- Drive it with JSON lines on `127.0.0.1:PORT`:
  - `{"id":1,"method":"cmdline.input","params":{"text":"circle 0,0 5"}}` — type at the command line exactly like a user (prompts, keywords, `@dx,dy`, `@d<a`, direct distance entry).
  - `{"id":2,"method":"engine.execute","params":{"command":"line","params":{"points":[[0,0],[10,0]]}}}` — JSON form.
  - `{"id":3,"method":"ui.screenshot","params":{"path":"/tmp/shot.png"}}` then read the PNG.
  - Methods: `crates/ui-egui/src/control.rs`; protocol doc: `docs/control-protocol.md`.
- **For UI work, look at the result** (screenshot, read the PNG) and compare with `plan/autocad/screenshots/`. If no frame is presented (screen locked) use `ui.render` or `redrob-cad-cli`.
- Headless: `redrob-cad-cli run --sample --script 'LINE 0,0 10,10\n' --export out.png`; `redrob-cad-cli info file.dxf`; `redrob-cad-cli mcp`.
- Shell gotcha: `mv`/`cp` may be aliased interactive — use `/bin/mv -f` / `/bin/cp -f`.
- Parallel agents: separate `CARGO_TARGET_DIR` per agent; edit only the crates you own; delete your target dir when done (disk).

## Roadmap
`ROADMAP.md` (committed) tracks status, milestones, parity and estimates. Update it whenever a milestone task lands. `cargo xtask parity` recomputes the command-catalog parity in `docs/parity.md`.
