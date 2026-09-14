# OpenCAD architecture

OpenCAD is a compatibility-preserving modernization of FreeCAD. It carries the complete upstream runtime instead of replacing proven CAD behavior with placeholders. The product surface is modernized in native Qt, and RedRob Code is added through the same Python-workbench extension system used by existing modules.

## Runtime layers

| Layer | Implementation | Compatibility rule |
|---|---|---|
| Geometry | OpenCASCADE through FreeCAD Part | Keep exact B-rep, topology, tessellation, and exchange behavior. |
| Application core | `FreeCADBase` and `FreeCADApp` | Keep document objects, properties, expressions, dependency recompute, transactions, persistence, and Python names. |
| Product modules | Complete `src/Mod` tree | Keep each build flag, dependency, command ID, object type, view provider, and file handler. |
| 3D interaction | Coin3D/Quarter/OpenGL | Keep selection paths, edit draggers, Pivy, navigation modes, cameras, screenshots, and workbench overlays. |
| Desktop interface | `FreeCADGui` on Qt with the OpenCAD theme and `branding.xml` | Modernize presentation without changing FreeCAD's executable/profile identity or adding Electron, Chromium, or a web runtime. Existing task panels and dialogs remain native. |
| Automation | Existing `FreeCAD`/`FreeCADGui` Python API | Existing macros, FeaturePython objects, workbenches, and add-ons remain valid. |
| AI | `src/Mod/OpenCADAI` | Use RedRob Code's local service as the agent engine; send bounded document context and execute generated Python only after user review. |

## Why this is a complete port rather than a clean-room rewrite

A verbatim language rewrite would not be behaviorally verbatim: FreeCAD's public behavior includes millions of lines of C++, Python callbacks, Qt task panels, Coin scene nodes, file handlers, and third-party integrations. OpenCAD therefore imports the complete implementation and changes it in place. This makes feature retention measurable: an upstream feature is present unless a tracked OpenCAD change removes it. A Rust rewrite would begin with near-zero parity and could not replace OpenCASCADE.

## Lightweight UI strategy

The OpenCAD appearance uses the existing parameterized QSS engine and native widgets. It adds only a YAML token set and preference pack, so memory remains dominated by CAD geometry rather than a bundled browser. New user profiles persist the OpenCAD stylesheet and theme before theme observers start; existing profiles with an explicit stylesheet retain it. Applying the built-in OpenCAD preference pack also sets the associated viewport and tree presentation values. All legacy workbench interfaces continue to use the unchanged widget hierarchy.

## RedRob execution flow

1. The OpenCAD AI dock asks an API-capable RedRob Code v2 CLI to start its authenticated local service.
2. It uses the CLI's local API bridge to create a session against a user-selected project directory. Startup failures stop the request and are reported in the dock.
3. The active document snapshot contains document metadata, up to 250 object summaries, and current selection—not full proprietary geometry or credentials.
4. RedRob runs the selected `plan` or `build` agent and returns its session context. Changing agent, model, or directory creates a correctly rebound session.
5. If the response contains FreeCAD Python, OpenCAD exposes a separate **Run reviewed Python** action.
6. Execution requires confirmation. Changes to the document that was active at execution start are committed only after script execution, recompute, and GUI refresh succeed; other documents and filesystem effects remain outside that transaction boundary.

RedRob remains the credential owner. OpenCAD does not request, persist, log, or transmit a workspace API key itself. Installation and authentication follow [RedRob Code's official instructions](https://console.redrob.ai/code). The adapter targets the current v2 `service`/`api` bridge around RedRob's local HTTP contract and reports an actionable compatibility error if an older CLI lacks those commands.

## Upstream synchronization

`freecad-source` is retained as a Git remote locally during development. Future updates should merge an upstream snapshot, resolve only real conflicts, run upstream tests, and regenerate the parity audit. OpenCAD-specific code is intentionally isolated to theme/branding changes and `src/Mod/OpenCADAI` to reduce divergence.

Content about RedRob Code was rephrased from its official documentation for compliance with licensing restrictions.
