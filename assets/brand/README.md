# Brand sources

Verbatim copies of the Redrob CAD app icon from the design system package
`@redrob-labs/ui@1.8.0` (`dist/brand/app-icons/redrob-cad/`). `SHA256SUMS` pins them (the same
hashes as that package's `manifest.json`), and `packaging/brand-assets.sh` derives everything
under `assets/app-icon/` from these and nothing else.

Rules that bind here:

- Never redrawn, recolored, re-proportioned, given a shadow, border or second radius.
- `redrob-cad-icon-web.svg` (rounded tile, transparent corners) is the source of every desktop
  icon: hicolor PNGs, window icon, `.ico`, `.icns`, the launcher PNG and the web favicon. Desktop
  shells and the launcher show the PNG unmasked, so the full-bleed `icon` tile is not used.
- The package's other variants (`icon`, `icon-dark`, `icon-web-dark`, `symbol`) are not shipped
  here: the app has no maskable/adaptive icon and one favicon.
- SVG is rendered with Chromium (`CHROME=… packaging/brand-assets.sh`). ImageMagick's SVG reader
  drops the gradients, so it is never used to render; it only resizes the rendered PNG down.

To update: copy the new SVGs from a newer `@redrob-labs/ui`, update `SHA256SUMS`, rerun the
script.
