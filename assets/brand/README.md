# Brand sources

Verbatim copies of the Redrob design system's shipped favicon files
(`redrob-design-system/assets/Favicons/`). `SHA256SUMS` pins them, and
`packaging/brand-assets.sh` derives everything under `assets/app-icon/` from these and nothing else.

Rules that bind here (design system `10-logo.md` and `assets/Favicons/README.md`):

- A white symbol on a Redrob Blue (`#2B52FF`) tile. One design, no dark-mode variant.
- Never redrawn, recolored, re-proportioned, given a shadow, border or second radius.
- Each size is drawn for itself. Where the design system ships a size it is used as is; any other
  size is the next larger tile resized down. Nothing is upscaled, so no icon here is larger than 512.
- `symbol-white-*.png` is the symbol alone on transparent, for a dark ground that is not Redrob Blue.

Redrob CAD has no product glyph of its own in the design system yet, so the app carries the Redrob
tile. When a CAD glyph is added to `assets/Products/`, replace these sources, update `SHA256SUMS`
and rerun the script; nothing else changes.
