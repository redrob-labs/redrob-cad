# FreeCAD ↔ OpenCAD feature parity

**Audit basis:** the OpenCAD branch is based directly on the current FreeCAD `main` source snapshot. “Source retained” means the same implementation is present in OpenCAD, not that a replacement merely resembles it. Runtime availability and behavior still depend on the same optional libraries, operating-system integrations, build flags, external solvers, and successful execution of the relevant upstream tests.

## Core and cross-cutting systems

| Feature family | FreeCAD implementation | OpenCAD implementation | Status |
|---|---|---|---|
| Runtime foundation | `src/Base`: types, persistence, units, parameters, Python interpreter, logging, files, XML/ZIP, exceptions, threading | Same source; runtime verification pending | Source retained |
| Parametric document model | `src/App`: documents, objects, dynamic/static properties, extensions, links, expressions | Same `FreeCADApp` implementation | Source retained |
| Dependency graph and recompute | Touched-state propagation, topological ordering, Python/GIL callbacks, error state | Same recompute engine | Source retained |
| Transactions and undo/redo | Cross-document transaction IDs, object/property change records | Same transaction engine and GUI boundaries | Source retained |
| Native project format | FCStd ZIP, `Document.xml`, `GuiDocument.xml`, auxiliary streams, thumbnail | Same readers/writers and schema | Source retained |
| Python automation | `FreeCAD`, `FreeCADGui`, generated bindings, FeaturePython, ViewProviderPython, macros | Same module names and initialization paths | Source retained |
| Add-ons | Legacy `Init.py`/`InitGui.py`, package-style `freecad.*`, `package.xml`, Addon Manager | Same discovery, precedence, metadata, and loading | Source retained |
| Commands/workbenches | Native and Python command registry, menus, toolbars, shortcuts, macro recording | Same IDs and command manager; OpenCAD AI adds commands | Source retained + extended |
| Preferences | XML parameter tree, preference pages/packs, per-workbench settings | Same storage/API; OpenCAD pack added | Source retained + modernized |
| Internationalization | Qt translations and workbench catalogs | Same translation resources | Source retained |
| Accessibility/input | Keyboard shortcuts, touch, mouse navigation, 3Dconnexion where built | Same Qt and device integrations | Source retained |
| 3D viewport | Coin3D, Quarter, OpenGL, view providers, selection, draggers, overlays | Same interactive viewport | Source retained |
| Navigation | CAD, Blender, Revit, NX, SolidWorks, Maya, OpenCascade, OpenSCAD, TinkerCAD, touch/gesture styles | Same navigation implementations | Source retained |
| Headless mode | `FreeCADCmd`, `import FreeCAD`, GUI-off builds | Same targets and App runtime | Source retained |
| Desktop UI | Qt widgets, docks, task panels, property editor, MDI, dialogs | Same inherited UI with lightweight OpenCAD native theme and supported `branding.xml` presentation | Modernized; inherited source retained |
| AI agent | None built into upstream core | RedRob Code workbench, context bridge, session UI, reviewed Python runner | New |
| Tests | Base/App/Gui/module GTest, Python tests, Qt/offscreen and visual tests | Complete upstream suites retained | Source retained; environment-dependent execution |

## Bundled modules and workbenches

| Module/workbench | FreeCAD feature set | OpenCAD side-by-side result | Status |
|---|---|---|---|
| AddonManager | Search, install, update, remove, compatibility and Python dependency management | Same module/submodule integration | Source retained when submodule is initialized |
| Assembly | Components, joints, constraints, assembly solver and related task panels | Same C++/Python module and UI | Source retained |
| BIM | Architectural/BIM objects, IFC workflows, building elements and drafting integration | Same built-in BIM module | Source retained |
| CAM | Jobs, tool controllers, toolbits, operations, path generation, simulation and post-processing | Same CAM engine, UI and posts | Source retained |
| Draft | 2D drafting, snapping, annotation, arrays, modification and working planes | Same Python workbench and commands | Source retained |
| Fem | Analysis objects, materials, meshing, solver interfaces, constraints and results | Same FEM stack; external solver availability unchanged | Source retained |
| Help | Integrated and online help, language paths and preferences | Same Help module | Source retained |
| Import | General CAD exchange and colored STEP/import services | Same handlers | Source retained |
| Inspection | Shape/mesh inspection and comparison workflows | Same module | Source retained |
| JtReader | JT ingestion | Same optional reader | Source retained when built |
| Material | Material cards, properties, editor and shared services | Same material system | Source retained |
| Measure | Distance, angle, area and related visual measurement tools | Same native module | Source retained |
| Mesh | Mesh objects, primitives, evaluation, repair and conversion | Same mesh kernel and UI | Source retained |
| MeshPart | Mesh/Part conversion and section/intersection workflows | Same bridge module | Source retained |
| OpenSCAD | CSG/OpenSCAD import, export and executable integration | Same workbench; external OpenSCAD requirement unchanged | Source retained |
| Part | OpenCASCADE topology, primitives, booleans, refinement, sectioning and shape operations | Same Part core and GUI | Source retained |
| PartDesign | Bodies, additive/subtractive features, patterns, dress-ups and feature history | Same module and task panels | Source retained |
| Plot | Python plotting integration | Same optional module | Source retained |
| Points | Point-cloud object model, import/export and operations | Same module | Source retained |
| ReverseEngineering | Geometry reconstruction from sampled/mesh data | Same workbench | Source retained |
| Robot | Robot models, trajectories and simulation-oriented tools | Same Eigen-dependent module | Source retained when built |
| Show | Shared scene/display helpers used by modules and scripts | Same support module | Source retained |
| Sketcher | 2D geometry, dimensional/geometric constraints, solver and interactive edit mode | Same solver, commands, view providers and task UI | Source retained |
| Spreadsheet | Cells, aliases, expressions, styling, import/export and table editor | Same module | Source retained |
| Start | Start page, recent files and onboarding surface | Same module under OpenCAD branding | Source retained |
| Surface | Surface filling, sections and curve/surface construction tools | Same workbench | Source retained |
| TechDraw | Pages, views, projections, sections, dimensions, annotations, templates and export | Same module | Source retained |
| Test | Python/GUI test harness and test workbench | Same test module | Source retained when enabled |
| Tux | Persistent toolbars, navigation indicator and compatibility helpers | Same utility module | Source retained |
| Web | Embedded web view/integration features | Same optional module | Source retained when built |
| TemplatePyMod | Developer template, disabled upstream by default | Remains disabled by default | Exact upstream state |
| OpenCADAI | Not present | RedRob-backed CAD assistant workbench | New |

## User-interface surfaces

| Surface | FreeCAD | OpenCAD | Status |
|---|---|---|---|
| Main menu and toolbars | Workbench-populated Qt actions | Same actions and customization under modern tokens | Source retained |
| Combo/tree view | Model tree, task tabs, drag/drop, context menus | Same native view with increased spacing and contrast | Modernized |
| Property editor | Data/View properties, units, expressions, editors | Same delegates and models | Source retained |
| Task panels | Workbench-specific interactive QWidget panels | Same panels | Source retained |
| Python console/editor | Console, tracing, macros, syntax highlighting | Same implementation | Source retained |
| Report/selection/DAG views | Dockable diagnostic and navigation views | Same docks | Source retained |
| MDI/document tabs | 3D, text, spreadsheet, TechDraw, web and split views | Same MDI contracts | Source retained |
| Preferences/customization | Themes, shortcuts, toolbar and workbench management | Same dialogs plus OpenCAD built-in theme | Extended |
| Render/selection | Display modes, clipping, preselection, subelements, screenshots | Same Coin scene and view-provider mapping | Source retained |
| AI dock | N/A | Dockable chat, plan/build selector, context inclusion, interruption, settings and reviewed script execution | New |

## File and data interoperability

| Family | FreeCAD | OpenCAD | Status |
|---|---|---|---|
| Native | FCStd and backup/recovery behavior | Same | Source retained |
| B-rep/CAD | BREP, STEP/STP, IGES/IGS and zipped STEP where enabled | Same handlers | Source retained |
| Mesh/scene | STL, OBJ, OFF, PLY, SMF, 3MF, Inventor, VRML, X3D, glTF and WebGL pathways | Same handlers | Source retained |
| 2D/vector | DXF, DWG through configured backend, SVG, OCA and airfoil formats | Same handlers/dependencies | Source retained |
| BIM/architecture | IFC, IFCJSON, Collada/DAE, 3DS, Sweet Home 3D and shapefile pathways | Same handlers/dependencies | Source retained |
| FEM/analysis | BDF, DAT, INP, MED, UNV, VTK/VTU/PVTU, Z88, FRD, FEniCS, YAML and JSON pathways | Same handlers/dependencies | Source retained |
| Manufacturing | G-code/post outputs and CAM-specific data | Same CAM posts and exporters | Source retained |
| Drawings/data | PDF, TechDraw outputs, spreadsheet CSV and image formats supported by Qt | Same handlers | Source retained |
| Point cloud/JT/PLM | Point formats, JT and PLMXML where optional components are built | Same optional handlers | Source retained when built |

The exact file-filter list remains registration-driven (`addImportType`/`addExportType`) in each module, just as in FreeCAD. This avoids inventing support claims for optional handlers unavailable in a particular build.

## Parity conclusion

OpenCAD does not intentionally remove or replace an inherited FreeCAD capability: the upstream implementation is retained at commit `d1424838c2cf9858f7b35cf6b774cfa07f7ba515`, with isolated branding, first-run theme, and AI additions listed in this repository's diff. That establishes source retention, not automatic proof of every runtime path. Binary/runtime parity for a package remains conditional on equivalent optional dependencies and successful upstream test execution.

## Validation record for this change

- Target base and independent reference clone resolve to the same upstream commit.
- All 31 module registrations, including the new OpenCAD AI module, were checked in `src/Mod/CMakeLists.txt`.
- New Python files pass syntax compilation and Ruff; XML preference files and all 66 theme tokens parse structurally; `git diff --check` passes.
- CMake configuration reached dependency discovery but could not complete in the sandbox because no Qt 5/6 development installation is available.
- The full upstream C++/GUI/module suite and a live authenticated RedRob session were therefore **not executed in this environment**. They remain release gates, not completed evidence.

This audit does not claim that optional external solvers/SDKs are bundled, that every add-on is compatible, or that retained tests have passed until a dependency-complete build runs them.
